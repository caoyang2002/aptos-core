// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Test infrastructure for modeling Aptos accounts.

use crate::gas_costs;
use aptos_crypto::ed25519::*;
use aptos_keygen::KeyGen;
use aptos_types::{
    access_path::AccessPath,
    account_address::AccountAddress,
    account_config::{self, AccountResource, CoinStoreResource},
    chain_id::ChainId,
    event::{EventHandle, EventKey},
    keyless::AnyKeylessPublicKey,
    state_store::state_key::StateKey,
    transaction::{
        authenticator::AuthenticationKey, EntryFunction, RawTransaction, Script, SignedTransaction,
        TransactionPayload,
    },
    write_set::{WriteOp, WriteSet, WriteSetMut},
};
use aptos_vm_genesis::GENESIS_KEYPAIR;
use move_core_types::move_resource::MoveStructType;

// TTL is 86400s. Initial time was set to 0.
pub const DEFAULT_EXPIRATION_TIME: u64 = 4_000_000;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AccountPublicKey {
    Ed25519(Ed25519PublicKey),
    // TODO: Do not expose this directly here since we'd have to make up for it in to_bytes below (AFAICT).
    //  instead, expose an AnyPublicKey?
    Keyless(AnyKeylessPublicKey),
}

impl AccountPublicKey {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            AccountPublicKey::Ed25519(pk) => pk.to_bytes().to_vec(),
            AccountPublicKey::Keyless(pk) => {
                // TODO: I don't think this should be called on a keyless AccountPublicKey until we
                //  refactor AccountPublicKey to actually contain an AnyPublicKey? I believe, higher
                //  level callers would expect a serialized AnyPublicKey that contains a keyless PK.
                match pk {
                    AnyKeylessPublicKey::Normal(pk) => pk.to_bytes(),
                    AnyKeylessPublicKey::Federated(pk) => pk.to_bytes(),
                }
            },
        }
    }

    pub fn as_ed25519(&self) -> Option<Ed25519PublicKey> {
        match self {
            AccountPublicKey::Ed25519(pk) => Some(pk.clone()),
            AccountPublicKey::Keyless(_) => None,
        }
    }

    pub fn as_keyless(&self) -> Option<AnyKeylessPublicKey> {
        match self {
            AccountPublicKey::Keyless(pk) => Some(pk.clone()),
            AccountPublicKey::Ed25519(_) => None,
        }
    }
}

/// Details about a Aptos account.
///
/// Tests will typically create a set of `Account` instances to run transactions on. This type
/// encodes the logic to operate on and verify operations on any Aptos account.
///
/// TODO: This is pleistocene-age code must be brought up to speed, since our accounts are not just Ed25519-based.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Account {
    addr: AccountAddress,
    /// The current private key for this account.
    /// TODO: When `pubkey` is of type `AccountPublicKey::Keyless`, this will be undefined.
    pub privkey: Ed25519PrivateKey,
    /// The current public key for this account.
    pub pubkey: AccountPublicKey,
}

impl Account {
    /// Creates a new account in memory.
    ///
    /// The account returned by this constructor is a purely logical entity, meaning that it does
    /// not automatically get added to the Aptos store. To add an account to the store, use
    /// [`AccountData`] instances with
    /// [`FakeExecutor::add_account_data`][crate::executor::FakeExecutor::add_account_data].
    /// This function returns distinct values upon every call.
    pub fn new() -> Self {
        let (privkey, pubkey) = KeyGen::from_os_rng().generate_ed25519_keypair();
        Self::with_keypair(privkey, pubkey)
    }

    /// Creates a new account in memory given a random seed.
    pub fn new_from_seed(seed: &mut KeyGen) -> Self {
        let (privkey, pubkey) = seed.generate_ed25519_keypair();
        Self::with_keypair(privkey, pubkey)
    }

    /// Creates an account with a specific address
    /// TODO: Currently stores a dummy SK/PK pair.
    pub fn new_from_addr(addr: AccountAddress, pubkey: AccountPublicKey) -> Self {
        let (privkey, _) = KeyGen::from_os_rng().generate_ed25519_keypair();
        Self {
            addr,
            privkey,
            pubkey,
        }
    }

    /// Creates a new account with the given keypair.
    ///
    /// Like with [`Account::new`], the account returned by this constructor is a purely logical
    /// entity.
    pub fn with_keypair(privkey: Ed25519PrivateKey, pubkey: Ed25519PublicKey) -> Self {
        let addr = aptos_types::account_address::from_public_key(&pubkey);
        Account {
            addr,
            privkey,
            pubkey: AccountPublicKey::Ed25519(pubkey),
        }
    }

    /// Creates a new account with the given addr and key pair
    ///
    /// Like with [`Account::new`], the account returned by this constructor is a purely logical
    /// entity.
    pub fn new_validator(
        addr: AccountAddress,
        privkey: Ed25519PrivateKey,
        pubkey: Ed25519PublicKey,
    ) -> Self {
        Account {
            addr,
            privkey,
            pubkey: AccountPublicKey::Ed25519(pubkey),
        }
    }

    /// Creates a new account in memory representing an account created in the genesis transaction.
    ///
    /// The address will be `address`, which should be an address for a genesis account and
    /// the account will use [`GENESIS_KEYPAIR`][static@@GENESIS_KEYPAIR] as its keypair.
    pub fn new_genesis_account(address: AccountAddress) -> Self {
        Account {
            addr: address,
            pubkey: AccountPublicKey::Ed25519(GENESIS_KEYPAIR.1.clone()),
            privkey: GENESIS_KEYPAIR.0.clone(),
        }
    }

    /// Creates a new account representing the aptos root account in memory.
    ///
    /// The address will be [`aptos_test_root_address`][account_config::aptos_test_root_address], and
    /// the account will use [`GENESIS_KEYPAIR`][static@GENESIS_KEYPAIR] as its keypair.
    pub fn new_aptos_root() -> Self {
        Self::new_genesis_account(account_config::aptos_test_root_address())
    }

    /// Returns the address of the account. This is a hash of the public key the account was created
    /// with.
    ///
    /// The address does not change if the account's [keys are rotated][Account::rotate_key].
    pub fn address(&self) -> &AccountAddress {
        &self.addr
    }

    /// Returns the AccessPath that describes the Account resource instance.
    ///
    /// Use this to retrieve or publish the Account blob.
    pub fn make_account_access_path(&self) -> AccessPath {
        AccessPath::resource_access_path(self.addr, AccountResource::struct_tag())
            .expect("access path in test")
    }

    /// Returns the AccessPath that describes the Account's CoinStore resource instance.
    ///
    /// Use this to retrieve or publish the Account CoinStore blob.
    pub fn make_coin_store_access_path(&self) -> AccessPath {
        AccessPath::resource_access_path(self.addr, CoinStoreResource::struct_tag())
            .expect("access path in  test")
    }

    /// Changes the keys for this account to the provided ones.
    pub fn rotate_key(&mut self, privkey: Ed25519PrivateKey, pubkey: Ed25519PublicKey) {
        self.privkey = privkey;
        self.pubkey = AccountPublicKey::Ed25519(pubkey);
    }

    /// Computes the authentication key for this account, as stored on the chain.
    ///
    /// This is the same as the account's address if the keys have never been rotated.
    pub fn auth_key(&self) -> Vec<u8> {
        match &self.pubkey {
            AccountPublicKey::Ed25519(pk) => AuthenticationKey::ed25519(pk),
            AccountPublicKey::Keyless(pk) => AuthenticationKey::any_key(pk.clone().into()),
        }
        .to_vec()
    }

    pub fn transaction(&self) -> TransactionBuilder {
        TransactionBuilder::new(self.clone())
    }
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TransactionBuilder {
    pub sender: Account,
    pub secondary_signers: Vec<Account>,
    pub fee_payer: Option<Account>,
    pub sequence_number: Option<u64>,
    pub program: Option<TransactionPayload>,
    pub max_gas_amount: Option<u64>,
    pub gas_unit_price: Option<u64>,
    pub chain_id: Option<ChainId>,
    pub ttl: Option<u64>,
}

impl TransactionBuilder {
    pub fn new(sender: Account) -> Self {
        Self {
            sender,
            secondary_signers: Vec::new(),
            fee_payer: None,
            sequence_number: None,
            program: None,
            max_gas_amount: None,
            gas_unit_price: None,
            chain_id: None,
            ttl: None,
        }
    }

    pub fn secondary_signers(mut self, secondary_signers: Vec<Account>) -> Self {
        self.secondary_signers = secondary_signers;
        self
    }

    pub fn fee_payer(mut self, fee_payer: Account) -> Self {
        self.fee_payer = Some(fee_payer);
        self
    }

    pub fn sequence_number(mut self, sequence_number: u64) -> Self {
        self.sequence_number = Some(sequence_number);
        self
    }

    pub fn chain_id(mut self, id: ChainId) -> Self {
        self.chain_id = Some(id);
        self
    }

    pub fn payload(mut self, payload: TransactionPayload) -> Self {
        self.program = Some(payload);
        self
    }

    pub fn script(mut self, s: Script) -> Self {
        self.program = Some(TransactionPayload::Script(s));
        self
    }

    pub fn entry_function(mut self, f: EntryFunction) -> Self {
        self.program = Some(TransactionPayload::EntryFunction(f));
        self
    }

    pub fn max_gas_amount(mut self, max_gas_amount: u64) -> Self {
        self.max_gas_amount = Some(max_gas_amount);
        self
    }

    pub fn gas_unit_price(mut self, gas_unit_price: u64) -> Self {
        self.gas_unit_price = Some(gas_unit_price);
        self
    }

    pub fn ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn raw(&self) -> RawTransaction {
        RawTransaction::new(
            *self.sender.address(),
            self.sequence_number.expect("sequence number not set"),
            self.program.clone().expect("transaction payload not set"),
            self.max_gas_amount.unwrap_or(gas_costs::TXN_RESERVED),
            self.gas_unit_price.unwrap_or(0),
            self.ttl.unwrap_or(DEFAULT_EXPIRATION_TIME),
            self.chain_id.unwrap_or_else(ChainId::test), //ChainId::test(),
        )
    }

    pub fn sign(self) -> SignedTransaction {
        self.raw()
            .sign(
                &self.sender.privkey,
                self.sender.pubkey.as_ed25519().unwrap(),
            )
            .unwrap()
            .into_inner()
    }

    pub fn sign_multi_agent(self) -> SignedTransaction {
        let secondary_signer_addresses: Vec<AccountAddress> = self
            .secondary_signers
            .iter()
            .map(|signer| *signer.address())
            .collect();
        let secondary_private_keys: Vec<&Ed25519PrivateKey> = self
            .secondary_signers
            .iter()
            .map(|signer| &signer.privkey)
            .collect();
        self.raw()
            .sign_multi_agent(
                &self.sender.privkey,
                secondary_signer_addresses,
                secondary_private_keys,
            )
            .unwrap()
            .into_inner()
    }

    pub fn sign_fee_payer(self) -> SignedTransaction {
        let secondary_signer_addresses: Vec<AccountAddress> = self
            .secondary_signers
            .iter()
            .map(|signer| *signer.address())
            .collect();
        let secondary_private_keys: Vec<&Ed25519PrivateKey> = self
            .secondary_signers
            .iter()
            .map(|signer| &signer.privkey)
            .collect();
        let fee_payer = self.fee_payer.clone().unwrap();
        self.raw()
            .sign_fee_payer(
                &self.sender.privkey,
                secondary_signer_addresses,
                secondary_private_keys,
                *fee_payer.address(),
                &fee_payer.privkey,
            )
            .unwrap()
            .into_inner()
    }
}

//---------------------------------------------------------------------------
// CoinStore resource representation
//---------------------------------------------------------------------------

/// Struct that represents an account CoinStore resource for tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoinStore {
    coin: u64,
    frozen: bool,
    deposit_events: EventHandle,
    withdraw_events: EventHandle,
}

impl CoinStore {
    /// Create a new CoinStore
    pub fn new(coin: u64, deposit_events: EventHandle, withdraw_events: EventHandle) -> Self {
        Self {
            coin,
            frozen: false,
            deposit_events,
            withdraw_events,
        }
    }

    /// Retrieve the balance inside of this
    pub fn coin(&self) -> u64 {
        self.coin
    }

    /// Returns the Move Value for the account's CoinStore
    pub fn to_bytes(&self) -> Vec<u8> {
        let coin_store = CoinStoreResource::new(
            self.coin,
            self.frozen,
            self.deposit_events.clone(),
            self.withdraw_events.clone(),
        );
        bcs::to_bytes(&coin_store).unwrap()
    }
}

//---------------------------------------------------------------------------
// Account resource representation
//---------------------------------------------------------------------------

/// Represents an account along with initial state about it.
///
/// `AccountData` captures the initial state needed to create accounts for tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountData {
    account: Account,
    sequence_number: u64,
    coin_register_events: EventHandle,
    key_rotation_events: EventHandle,
    coin_store: CoinStore,
}

fn new_event_handle(count: u64, address: AccountAddress) -> EventHandle {
    EventHandle::new(EventKey::new(count, address), 0)
}

impl AccountData {
    /// Creates a new `AccountData` with a new account.
    ///
    /// This constructor is non-deterministic and should not be used against golden file.
    pub fn new(balance: u64, sequence_number: u64) -> Self {
        Self::with_account(Account::new(), balance, sequence_number)
    }

    pub fn increment_sequence_number(&mut self) {
        self.sequence_number += 1;
    }

    /// Creates a new `AccountData` with a new account.
    ///
    /// Most tests will want to use this constructor.
    pub fn new_from_seed(seed: &mut KeyGen, balance: u64, sequence_number: u64) -> Self {
        Self::with_account(Account::new_from_seed(seed), balance, sequence_number)
    }

    /// Creates a new `AccountData` with the provided account.
    pub fn with_account(account: Account, balance: u64, sequence_number: u64) -> Self {
        Self::with_account_and_event_counts(account, balance, sequence_number, 0, 0)
    }

    /// Creates a new `AccountData` with the provided account.
    pub fn with_keypair(
        privkey: Ed25519PrivateKey,
        pubkey: Ed25519PublicKey,
        balance: u64,
        sequence_number: u64,
    ) -> Self {
        let account = Account::with_keypair(privkey, pubkey);
        Self::with_account(account, balance, sequence_number)
    }

    /// Creates a new `AccountData` with custom parameters.
    pub fn with_account_and_event_counts(
        account: Account,
        balance: u64,
        sequence_number: u64,
        sent_events_count: u64,
        received_events_count: u64,
    ) -> Self {
        let addr = *account.address();
        Self {
            account,
            coin_store: CoinStore::new(
                balance,
                new_event_handle(received_events_count, addr),
                new_event_handle(sent_events_count, addr),
            ),
            sequence_number,
            coin_register_events: new_event_handle(0, addr),
            key_rotation_events: new_event_handle(1, addr),
        }
    }

    /// Changes the keys for this account to the provided ones.
    pub fn rotate_key(&mut self, privkey: Ed25519PrivateKey, pubkey: Ed25519PublicKey) {
        self.account.rotate_key(privkey, pubkey)
    }

    /// Creates and returns the top-level resources to be published under the account
    pub fn to_bytes(&self) -> Vec<u8> {
        let account = AccountResource::new(
            self.sequence_number,
            self.account.auth_key(),
            self.coin_register_events.clone(),
            self.key_rotation_events.clone(),
        );
        bcs::to_bytes(&account).unwrap()
    }

    /// Returns the AccessPath that describes the Account resource instance.
    ///
    /// Use this to retrieve or publish the Account blob.
    pub fn make_account_access_path(&self) -> AccessPath {
        self.account.make_account_access_path()
    }

    /// Returns the AccessPath that describes the Account's CoinStore resource instance.
    ///
    /// Use this to retrieve or publish the Account's CoinStore blob.
    pub fn make_coin_store_access_path(&self) -> AccessPath {
        self.account.make_coin_store_access_path()
    }

    /// Creates a writeset that contains the account data and can be patched to the storage
    /// directly.
    pub fn to_writeset(&self) -> WriteSet {
        let write_set = vec![
            (
                StateKey::resource_typed::<AccountResource>(self.address()).unwrap(),
                WriteOp::legacy_modification(self.to_bytes().into()),
            ),
            (
                StateKey::resource_typed::<CoinStoreResource>(self.address()).unwrap(),
                WriteOp::legacy_modification(self.coin_store.to_bytes().into()),
            ),
        ];

        WriteSetMut::new(write_set).freeze().unwrap()
    }

    /// Returns the address of the account. This is a hash of the public key the account was created
    /// with.
    ///
    /// The address does not change if the account's [keys are rotated][AccountData::rotate_key].
    pub fn address(&self) -> &AccountAddress {
        self.account.address()
    }

    /// Returns the underlying [`Account`] instance.
    pub fn account(&self) -> &Account {
        &self.account
    }

    /// Converts this data into an `Account` instance.
    pub fn into_account(self) -> Account {
        self.account
    }

    /// Returns the initial balance.
    pub fn balance(&self) -> u64 {
        self.coin_store.coin()
    }

    /// Returns the initial sequence number.
    pub fn sequence_number(&self) -> u64 {
        self.sequence_number
    }

    /// Returns the unique key for this sent events stream.
    pub fn sent_events_key(&self) -> &EventKey {
        self.coin_store.withdraw_events.key()
    }

    /// Returns the initial sent events count.
    pub fn sent_events_count(&self) -> u64 {
        self.coin_store.withdraw_events.count()
    }

    /// Returns the unique key for this received events stream.
    pub fn received_events_key(&self) -> &EventKey {
        self.coin_store.deposit_events.key()
    }

    /// Returns the initial received events count.
    pub fn received_events_count(&self) -> u64 {
        self.coin_store.deposit_events.count()
    }
}
