// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0
use crate::{call_custom_modules::{TransactionGeneratorWorker, UserModuleTransactionGenerator}, econia_order_generator, publishing::publish_util::Package, ObjectPool, ReliableTransactionSubmitter};
use aptos_sdk::{
    bcs,
    move_types::account_address::AccountAddress,
    transaction_builder::TransactionFactory,
    types::{transaction::SignedTransaction, LocalAccount},
};
use async_trait::async_trait;
use move_core_types::{
    ident_str,
    language_storage::ModuleId,
};
use aptos_types::transaction::{EntryFunction, TransactionPayload};
use rand::{rngs::StdRng, Rng};

use std::sync::Arc;
// use aptos_infallible::RwLock;

/// Placeas a bid limit order.
pub fn place_bid_limit_order(
    module_id: ModuleId,
    size: u64,
    price: u64,
    market_id: u64
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("place_bid_limit_order").to_owned(),
        vec![],
        vec![
            bcs::to_bytes(&size).unwrap(),
            bcs::to_bytes(&price).unwrap(),
            bcs::to_bytes(&market_id).unwrap(),
        ],
    ))
}

/// Placeas an ask limit order.
pub fn place_ask_limit_order(
    module_id: ModuleId,
    size: u64,
    price: u64,
    market_id: u64
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("place_ask_limit_order").to_owned(),
        vec![],
        vec![
            bcs::to_bytes(&size).unwrap(),
            bcs::to_bytes(&price).unwrap(),
            bcs::to_bytes(&market_id).unwrap(),
        ],
    ))
}

/// Placeas a bid market order.
pub fn place_bid_market_order(
    module_id: ModuleId,
    size: u64,
    market_id: u64
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("place_bid_market_order").to_owned(),
        vec![],
        vec![
            bcs::to_bytes(&size).unwrap(),
            bcs::to_bytes(&market_id).unwrap(),
        ],
    ))
}

/// Placeas an ask market order.
pub fn place_ask_market_order(
    module_id: ModuleId,
    size: u64,
    market_id: u64
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("place_ask_market_order").to_owned(),
        vec![],
        vec![
            bcs::to_bytes(&size).unwrap(),
            bcs::to_bytes(&market_id).unwrap(),
        ],
    ))
}

pub fn register_market(
    module_id: ModuleId,
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("register_market").to_owned(),
        vec![],
        vec![],
    ))
}

pub fn register_market_accounts(
    module_id: ModuleId,
    market_id: u64
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("register_market_accounts").to_owned(),
        vec![],
        vec![
            bcs::to_bytes(&market_id).unwrap(),
        ],
    ))
}

pub fn deposit_coins(
    module_id: ModuleId,
    market_id: u64
) -> TransactionPayload {
    TransactionPayload::EntryFunction(EntryFunction::new(
        module_id,
        ident_str!("deposit_coins").to_owned(),
        vec![],
        vec![
            bcs::to_bytes(&market_id).unwrap(),
        ],
    ))
}

pub struct EconiaLimitOrderTransactionGenerator {
    to_setup: Arc<ObjectPool<LocalAccount>>,
    done: Arc<ObjectPool<LocalAccount>>,
    num_base_orders_placed: usize,
    num_markets: Arc<u64>,
}

impl EconiaLimitOrderTransactionGenerator {
    pub fn new(
        to_setup: Arc<ObjectPool<LocalAccount>>,
        done: Arc<ObjectPool<LocalAccount>>,
        num_markets: u64
    ) -> Self {
        Self {
            to_setup,
            done,
            num_base_orders_placed: 0,
            num_markets: Arc::new(num_markets)
        }
    }
}

#[async_trait]
impl UserModuleTransactionGenerator for EconiaLimitOrderTransactionGenerator {
    fn initialize_package(
        &mut self,
        _package: &Package,
        _publisher: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _rng: &mut StdRng,
    ) -> Vec<SignedTransaction> {
        vec![]
    }

    async fn create_generator_fn(
        &mut self,
        _root_account: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _txn_executor: &dyn ReliableTransactionSubmitter,
        rng: &mut StdRng,
    ) -> Arc<TransactionGeneratorWorker> {
        let to_setup = self.to_setup.clone();
        let done = self.done.clone();
        let num_markets = self.num_markets.clone();
        self.num_base_orders_placed += 1;
        if self.num_base_orders_placed <= 100 || self.num_base_orders_placed % 2 == 0 {
            Arc::new(move |account, package, publisher, txn_factory, rng| {
                // Question: Is this correct? We are signing the transactions with `account`.
                // We are not using the batch sampled here.
                let batch = to_setup.take_from_pool(1, true, rng);
                if batch.is_empty() {
                    return vec![];
                }
                done.add_to_pool(batch);

                let mut requests = vec![];
                for market_id in 1..(*num_markets+1) {
                    let bid_size = rng.gen_range(2, 10);
                    let ask_size = rng.gen_range(2, 10);

                    let bid_price = rng.gen_range(1, 200);
                    let ask_price = rng.gen_range(201, 400);

                    let bid_builder = txn_factory.payload(place_bid_limit_order(package.get_module_id("txn_generator_utils"), bid_size, bid_price, market_id));
                    let ask_builder = txn_factory.payload(place_ask_limit_order(package.get_module_id("txn_generator_utils"), ask_size, ask_price, market_id));

                    requests.push(account.sign_with_transaction_builder(bid_builder));
                    requests.push(account.sign_with_transaction_builder(ask_builder));
                }
                requests
            })
        } else {
            Arc::new(move |account, package, publisher, txn_factory, rng| {
                let batch = to_setup.take_from_pool(1, true, rng);
                if batch.is_empty() {
                    return vec![];
                }
                done.add_to_pool(batch);

                let mut requests = vec![];
                for market_id in 1..(*num_markets+1) {
                    let bid_size = rng.gen_range(2, 10);
                    let ask_size = rng.gen_range(2, 10);

                    let bid_builder = txn_factory.payload(place_bid_market_order(package.get_module_id("txn_generator_utils"), bid_size, market_id));
                    let ask_builder = txn_factory.payload(place_ask_market_order(package.get_module_id("txn_generator_utils"), ask_size, market_id));

                    requests.push(account.sign_with_transaction_builder(bid_builder));
                    requests.push(account.sign_with_transaction_builder(ask_builder));
                }
                requests
            })
        }
    }
}


pub struct EconiaRegisterMarketTransactionGenerator {
    num_markets: Arc<u64>,
}

impl EconiaRegisterMarketTransactionGenerator {
    pub fn new(
        num_markets: u64
    ) -> Self {
        Self {
            num_markets: Arc::new(num_markets),
        }
    }
}

#[async_trait]
impl UserModuleTransactionGenerator for EconiaRegisterMarketTransactionGenerator {
    fn initialize_package(
        &mut self,
        _package: &Package,
        _publisher: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _rng: &mut StdRng,
    ) -> Vec<SignedTransaction> {
        vec![]
    }

    async fn create_generator_fn(
        &mut self,
        _root_account: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _txn_executor: &dyn ReliableTransactionSubmitter,
        rng: &mut StdRng,
    ) -> Arc<TransactionGeneratorWorker> {
        let num_markets = self.num_markets.clone();
        Arc::new(move |account, package, publisher, txn_factory, rng| {
            let mut requests = vec![];
            for i in 0..*num_markets {
                let builder = txn_factory.payload(register_market(package.get_module_id("txn_generator_utils")));
                requests.push(account.sign_with_transaction_builder(builder));
            }
            requests
        })
    }
}


pub struct EconiaRegisterMarketUserTransactionGenerator {
    num_markets: Arc<u64>,
}

impl EconiaRegisterMarketUserTransactionGenerator {
    pub fn new(
        num_markets: u64
    ) -> Self {
        Self {
            num_markets: Arc::new(num_markets),
        }
    }
}

#[async_trait]
impl UserModuleTransactionGenerator for EconiaRegisterMarketUserTransactionGenerator {
    fn initialize_package(
        &mut self,
        _package: &Package,
        _publisher: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _rng: &mut StdRng,
    ) -> Vec<SignedTransaction> {
        vec![]
    }

    async fn create_generator_fn(
        &mut self,
        _root_account: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _txn_executor: &dyn ReliableTransactionSubmitter,
        rng: &mut StdRng,
    ) -> Arc<TransactionGeneratorWorker> {
        let num_markets = self.num_markets.clone();
        Arc::new(move |account, package, publisher, txn_factory, rng| {
            let mut requests = vec![];
            for market_id in 1..(*num_markets+1) {
                let builder = txn_factory.payload(register_market_accounts(package.get_module_id("txn_generator_utils"), market_id));
                requests.push(account.sign_with_transaction_builder(builder));
            }
            requests
        })
    }
}



pub struct EconiaDepositCoinsTransactionGenerator {
    num_markets: Arc<u64>,
}

impl EconiaDepositCoinsTransactionGenerator {
    pub fn new(
        num_markets: u64
    ) -> Self {
        Self {
            num_markets: Arc::new(num_markets),
        }
    }
}

#[async_trait]
impl UserModuleTransactionGenerator for EconiaDepositCoinsTransactionGenerator {
    fn initialize_package(
        &mut self,
        _package: &Package,
        _publisher: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _rng: &mut StdRng,
    ) -> Vec<SignedTransaction> {
        vec![]
    }

    async fn create_generator_fn(
        &mut self,
        _root_account: &mut LocalAccount,
        _txn_factory: &TransactionFactory,
        _txn_executor: &dyn ReliableTransactionSubmitter,
        rng: &mut StdRng,
    ) -> Arc<TransactionGeneratorWorker> {
        let num_markets = self.num_markets.clone();
        Arc::new(move |account, package, publisher, txn_factory, rng| {
            let mut requests = vec![];
            for market_id in 1..(*num_markets+1) {
                let builder = txn_factory.payload(deposit_coins(package.get_module_id("txn_generator_utils"), market_id));
                requests.push(account.sign_multi_agent_with_transaction_builder(vec![publisher], builder))
            }
            requests
        })
    }
}