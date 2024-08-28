// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    counters::{NUM_NON_PREEXECUTED_BLOCKS, NUM_PREEXECUTED_BLOCKS}, pipeline::{
        execution_wait_phase::ExecutionWaitRequest,
        pipeline_phase::{CountedRequest, StatelessPipeline},
    }, state_computer::{PipelineExecutionResult, StateComputeResultFut, SyncStateComputeResultFut}, state_replication::StateComputer
};
use aptos_consensus_types::pipelined_block::PipelinedBlock;
use aptos_crypto::HashValue;
use aptos_executor_types::{ExecutorError, ExecutorResult};
use aptos_logger::{debug, info};
use async_trait::async_trait;
use dashmap::DashMap;
use futures::TryFutureExt;
use std::{
    collections::HashMap, fmt::{Debug, Display, Formatter}, pin::Pin, sync::Arc
};

/// [ This class is used when consensus.decoupled = true ]
/// ExecutionSchedulePhase is a singleton that receives ordered blocks from
/// the buffer manager and send them to the ExecutionPipeline.

pub struct ExecutionRequest {
    pub ordered_blocks: Vec<PipelinedBlock>,
    // Hold a CountedRequest to guarantee the executor doesn't get reset with pending tasks
    // stuck in the ExecutinoPipeline.
    pub lifetime_guard: CountedRequest<()>,
}

impl Debug for ExecutionRequest {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Display for ExecutionRequest {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "ExecutionScheduleRequest({:?})", self.ordered_blocks)
    }
}

pub struct ExecutionSchedulePhase {
    execution_proxy: Arc<dyn StateComputer>,
    execution_futures: Arc<DashMap<HashValue, SyncStateComputeResultFut>>,
}

impl ExecutionSchedulePhase {
    pub fn new(execution_proxy: Arc<dyn StateComputer>, execution_futures: Arc<DashMap<HashValue, SyncStateComputeResultFut>>) -> Self {
        Self {
            execution_proxy,
            execution_futures,
        }
    }
}

#[async_trait]
impl StatelessPipeline for ExecutionSchedulePhase {
    type Request = ExecutionRequest;
    type Response = ExecutionWaitRequest;

    const NAME: &'static str = "execution_schedule";

    async fn process(&self, req: ExecutionRequest) -> ExecutionWaitRequest {
        let ExecutionRequest {
            ordered_blocks,
            lifetime_guard,
        } = req;

        let block_id = match ordered_blocks.last() {
            Some(block) => block.id(),
            None => {
                return ExecutionWaitRequest {
                    block_id: HashValue::zero(),
                    fut: Box::pin(async { Err(aptos_executor_types::ExecutorError::EmptyBlocks) }),
                }
            },
        };

        // Call schedule_compute() for each block here (not in the fut being returned) to
        // make sure they are scheduled in order.
        for block in &ordered_blocks {
            match self.execution_futures.entry(block.id()) {
                dashmap::mapref::entry::Entry::Occupied(_) => {
                    info!("[PreExecution] block was pre-executed, epoch {} round {} id {}", block.epoch(), block.round(), block.id());
                    NUM_PREEXECUTED_BLOCKS.inc();
                }
                dashmap::mapref::entry::Entry::Vacant(entry) => {
                    info!("[PreExecution] block was not pre-executed, epoch {} round {} id {}", block.epoch(), block.round(), block.id());
                    let fut = self
                        .execution_proxy
                        .schedule_compute(block.block(), block.parent_id(), block.randomness().cloned())
                        .await;
                    entry.insert(fut);
                    NUM_NON_PREEXECUTED_BLOCKS.inc();
                }
            }
        }

        let execution_futures = self.execution_futures.clone();

        // In the future being returned, wait for the compute results in order.
        // n.b. Must `spawn()` here to make sure lifetime_guard will be released even if
        //      ExecutionWait phase is never kicked off.
        let fut = tokio::task::spawn(async move {
            let mut results = vec![];
            // wait for all futs so that lifetime_guard is guaranteed to be dropped only
            // after all executor calls are over
            for block in &ordered_blocks {
                debug!("[Execution] try to receive compute result for block, epoch {} round {} id {}", block.epoch(), block.round(), block.id());
                match execution_futures.entry(block_id) {
                    dashmap::mapref::entry::Entry::Occupied(entry) => {
                        let fut = entry.get().clone();
                        results.push(fut.await);
                    }
                    dashmap::mapref::entry::Entry::Vacant(_) => {
                        return Err(ExecutorError::internal_err(format!(
                            "Failed to find compute result for block {}",
                            block.id()
                        )));
                    }
                }
            }
            let results = itertools::zip_eq(ordered_blocks, results)
                .map(|(block, res)| {
                    let PipelineExecutionResult {
                        input_txns,
                        result,
                        execution_time,
                    } = res?;
                    Ok(block.set_execution_result(input_txns, result, execution_time))
                })
                .collect::<ExecutorResult<Vec<_>>>()?;
            drop(lifetime_guard);
            Ok(results)
        })
        .map_err(ExecutorError::internal_err)
        .and_then(|res| async { res });

        ExecutionWaitRequest {
            block_id,
            fut: Box::pin(fut),
        }
    }
}
