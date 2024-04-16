// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use aptos_config::config::{NetworkConfig, PeerRole};
use aptos_config::network_id::{NetworkContext, PeerNetworkId};
use aptos_netcore::transport::{ConnectionOrigin, Transport};
use aptos_network2::transport::Connection;
use aptos_network2::application::ApplicationCollector;
use aptos_network2::application::storage::PeersAndMetadata;
use aptos_network2::logging::NetworkSchema;
use aptos_network2::protocols::network::OutboundPeerConnections;
use aptos_types::network_address::NetworkAddress;
use tokio::runtime::Handle;
use aptos_logger::{error, info, warn};
use aptos_network2::{counters, peer};
use aptos_short_hex_str::AsShortHexStr;
use futures::{AsyncRead, AsyncWrite, AsyncWriteExt, StreamExt};
use std::marker::PhantomData;
use std::sync::Arc;
use aptos_time_service::{TimeService,TimeServiceTrait};

pub struct PeerListener<TTransport, TSocket>
    where
        TTransport: Transport,
        TSocket: AsyncRead + AsyncWrite,
{
    transport: TTransport,
    peers_and_metadata: Arc<PeersAndMetadata>,
    config: NetworkConfig,
    network_context: NetworkContext,
    apps: Arc<ApplicationCollector>,
    peer_senders: Arc<OutboundPeerConnections>,
    time_service: TimeService,
    _ph2 : PhantomData<TSocket>,
}

impl<TTransport, TSocket> PeerListener<TTransport, TSocket>
    where
        TTransport: Transport<Output = Connection<TSocket>> + Send + 'static,
        TSocket: aptos_network2::transport::TSocket,
{
    pub fn new(
        transport: TTransport,
        peers_and_metadata: Arc<PeersAndMetadata>,
        config: NetworkConfig,
        network_context: NetworkContext,
        apps: Arc<ApplicationCollector>,
        peer_senders: Arc<OutboundPeerConnections>,
        time_service: TimeService,
    ) -> Self {
        Self{
            transport,
            peers_and_metadata,
            config,
            network_context,
            apps,
            peer_senders,
            time_service,
            _ph2: Default::default(),
        }
    }

    pub(crate) fn listen(
        mut self,
        listen_addr: NetworkAddress,
        executor: Handle,
    ) -> Result<NetworkAddress, <TTransport>::Error> {
        let (sockets, listen_addr_actual) = executor.block_on(self.first_listen(listen_addr))?;
        info!("listener_thread to spawn ({:?})", listen_addr_actual);
        executor.spawn(self.listener_thread(sockets, executor.clone()));
        Ok(listen_addr_actual)
    }

    async fn first_listen(&mut self, listen_addr: NetworkAddress) -> Result<(<TTransport>::Listener, NetworkAddress), TTransport::Error> {
        self.transport.listen_on(listen_addr)
    }

    async fn listener_thread(mut self, mut sockets: <TTransport>::Listener, executor: Handle) {
        info!("listener_thread start");
        loop {
            let (conn_fut, remote_addr) = match sockets.next().await {
                Some(result) => match result {
                    Ok(conn) => { conn }
                    Err(err) => {
                        error!("listener_thread {:?} got err {:?}, exiting", self.config.network_id, err);
                        return;
                    }
                }
                None => {
                    error!("listener_thread {:?} got None, assuming source closed, exiting", self.config.network_id, );
                    return;
                }
            };
            // TODO: we could start a task here to handle connection negotiation the socket-listener could accept and start another connection
            let upgrade_start = self.time_service.now();
            match conn_fut.await {
                Ok(mut connection) => {
                    let elapsed_time = (self.time_service.now() - upgrade_start).as_secs_f64();
                    let ok = self.check_new_inbound_connection(&connection);
                    let counter_state = if ok {
                        counters::SUCCEEDED_LABEL
                    } else {
                        counters::FAILED_LABEL
                    };
                    counters::connection_upgrade_time(&self.network_context, ConnectionOrigin::Inbound, counter_state).observe(elapsed_time);
                    if !ok {
                        info!("listener_thread got connection {:?}, failed", remote_addr);
                        // counted and logged inside check function above, just close here and be done.
                        _ = connection.socket.close().await;
                        continue;
                    }
                    info!(
                        network_id = self.network_context.network_id().as_str(),
                        peer = connection.metadata.remote_peer_id,
                        "listener_thread got connection {:?}, ok!", remote_addr,
                    );
                    let remote_peer_network_id = PeerNetworkId::new(self.network_context.network_id(), connection.metadata.remote_peer_id);
                    peer::start_peer(
                        &self.config,
                        connection.socket,
                        connection.metadata,
                        self.apps.clone(),
                        executor.clone(),
                        remote_peer_network_id,
                        self.peers_and_metadata.clone(),
                        self.peer_senders.clone(),
                        self.network_context,
                        self.time_service.clone(),
                    );
                }
                Err(err) => {
                    info!(addr = remote_addr, "listener_thread {:?} connection post-processing failed (continuing): {:?}", self.config.network_id, err);
                }
            }
        }
    }

    // is the new inbound connection okay? => true
    // no, we should disconnect => false
    fn check_new_inbound_connection(&mut self, conn: &Connection<TSocket>) -> bool {
        // Everything below here is meant for unknown peers only. The role comes from
        // the Noise handshake and if it's not `Unknown` then it is trusted.
        if conn.metadata.role != PeerRole::Unknown {
            return true;
        }

        // Count unknown inbound connections
        let mut unknown_inbound_conns = 0;
        let mut already_connected = false;
        let remote_peer_id = conn.metadata.remote_peer_id;

        if remote_peer_id == self.network_context.peer_id() {
            debug_assert!(false, "Self dials shouldn't happen");
            warn!(
                NetworkSchema::new(&self.network_context)
                    .connection_metadata_with_address(&conn.metadata),
                "Received self-dial, disconnecting it"
            );
            return false;
        }

        // get a current count of all inbound connections, filter for maybe already being connected to the peer we are currently getting a connection from
        let pam_all = self.peers_and_metadata.get_all_peers_and_metadata();

        for (_network_id, netpeers) in pam_all.iter() {
            for (peer_id, peer_metadata) in netpeers.iter() {
                if !peer_metadata.is_connected() {
                    continue;
                }
                if *peer_id == remote_peer_id {
                    already_connected = true;
                }
                let remote_metadata = peer_metadata.get_connection_metadata();
                if remote_metadata.origin == ConnectionOrigin::Inbound && remote_metadata.role == PeerRole::Unknown {
                    unknown_inbound_conns += 1;
                }
            }
        }

        // Reject excessive inbound connections made by unknown peers
        // We control outbound connections with Connectivity manager before we even send them
        // and we must allow connections that already exist to pass through tie breaking.
        if !already_connected
            && unknown_inbound_conns + 1 > self.config.max_inbound_connections
        {
            info!(
                NetworkSchema::new(&self.network_context)
                .connection_metadata_with_address(&conn.metadata),
                "{} Connection rejected due to connection limit: {}",
                self.network_context,
                conn.metadata
            );
            counters::connections_rejected(&self.network_context, conn.metadata.origin).inc();
            return false;
        }

        if already_connected {
            // old code at network/framework/src/peer_manager/mod.rs PeerManager::add_peer() line 615 had provision for sometimes keeping the new connection, but this simplifies and always _drops_ the new connection
            info!(
                NetworkSchema::new(&self.network_context)
                .connection_metadata_with_address(&conn.metadata),
                "{} Closing incoming connection with Peer {} which is already connected",
                self.network_context,
                remote_peer_id.short_str()
            );
            false
        } else {
            true
        }
    }
}