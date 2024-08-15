// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::on_chain_config::BlockGasLimitType;
use serde::{Deserialize, Serialize};

/// Configuration for BlockSTM: determines behavior of the workers that rolling
/// commit transactions and may perform a 'backup' execution of the immediately
/// following transaction tx in order to make sure the critical path of the block
/// execution does not contain validation failure and re-execution of tx.
/// Note: currently, with a VM per worker, it also keeps the VM of the committer
/// worker warm, and more likely to have a faster back-up execution if the
/// original execution happens to be slow due to module loading.
///
/// The default setting is provided by the implementation whereby certain small
/// number of workers are designated committers: only these workers attempt to
/// commit transactions, and may perform the backup execution. Since committing
/// as early as possible is desired, these workers otherwise perform only tasks
/// close to the committed prefix, to avoid potentially long dependency waiting.
///
/// In the 'All' setting, all workers act as committers and may perform backup
/// executions, but they can also each perform any task (no 'close to the
/// committed prefix' restriction). This setting is used for testing with the
/// mock executor (that may pause some ongoing executions / workers) to ensure
/// that the commits (and backup executions) always happen.
///
/// In the 'None' setting, all workers act as committers, and backup is disabled.
/// Like 'All' there is no restriction on what tasks a worker can perform. This
/// setting is also intended for testing and debugging purposes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockSTMCommitterBackup {
    None,
    Default,
    All,
}

/// Local, per-node configuration.
#[derive(Clone, Debug)]
pub struct BlockExecutorLocalConfig {
    pub concurrency_level: usize,
    // If specified, parallel block execution fallbacks to sequential, if issue occurs.
    // If there is still an error in sequential block fallback as well, we will panic.
    pub allow_sequential_block_fallback: bool,
    // If true, we will discard the failed blocks and continue with the next block.
    // (allow_sequential_block_fallback needs to be set)
    pub discard_failed_blocks: bool,
    // When true, block-stm will record and log certain profiling outputs.
    pub enable_block_stm_profiling: bool,
    pub block_stm_committer_backup: BlockSTMCommitterBackup,
}

/// Configuration from on-chain configuration, that is
/// required to be the same across all nodes.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockExecutorConfigFromOnchain {
    pub block_gas_limit_type: BlockGasLimitType,
}

impl BlockExecutorConfigFromOnchain {
    pub fn new_no_block_limit() -> Self {
        Self {
            block_gas_limit_type: BlockGasLimitType::NoLimit,
        }
    }

    pub fn new_maybe_block_limit(maybe_block_gas_limit: Option<u64>) -> Self {
        Self {
            block_gas_limit_type: maybe_block_gas_limit
                .map_or(BlockGasLimitType::NoLimit, BlockGasLimitType::Limit),
        }
    }

    pub const fn on_but_large_for_test() -> Self {
        Self {
            block_gas_limit_type:
                // present, so code is excercised, but large to not limit blocks
                BlockGasLimitType::ComplexLimitV1 {
                    effective_block_gas_limit: 1_000_000_000,
                    execution_gas_effective_multiplier: 1,
                    io_gas_effective_multiplier: 1,
                    block_output_limit: Some(1_000_000_000_000),
                    conflict_penalty_window: 8,
                    use_module_publishing_block_conflict: true,
                    include_user_txn_size_in_block_output: true,
                    add_block_limit_outcome_onchain: false,
                    use_granular_resource_group_conflicts: false,
                },
        }
    }
}

/// Configuration for the BlockExecutor.
#[derive(Clone, Debug)]
pub struct BlockExecutorConfig {
    /// Local, per-node configuration.
    pub local: BlockExecutorLocalConfig,
    /// Configuration from on-chain configuration, that is
    /// required to be the same across all nodes.
    pub onchain: BlockExecutorConfigFromOnchain,
}

impl BlockExecutorConfig {
    pub fn new_no_block_limit(concurrency_level: usize) -> Self {
        Self {
            local: BlockExecutorLocalConfig {
                concurrency_level,
                allow_sequential_block_fallback: true,
                discard_failed_blocks: false,
                enable_block_stm_profiling: false,
                block_stm_committer_backup: BlockSTMCommitterBackup::Default,
            },
            onchain: BlockExecutorConfigFromOnchain::new_no_block_limit(),
        }
    }

    pub fn new_maybe_block_limit(
        concurrency_level: usize,
        maybe_block_gas_limit: Option<u64>,
    ) -> Self {
        Self {
            local: BlockExecutorLocalConfig {
                concurrency_level,
                allow_sequential_block_fallback: true,
                discard_failed_blocks: false,
                enable_block_stm_profiling: false,
                block_stm_committer_backup: BlockSTMCommitterBackup::Default,
            },
            onchain: BlockExecutorConfigFromOnchain::new_maybe_block_limit(maybe_block_gas_limit),
        }
    }
}
