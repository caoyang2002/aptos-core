// Copyright © Aptos Foundation

use crate::{
    sharded_block_executor::local_executor_shard::LocalExecutorShard,
    ShardedBlockExecutor
};
use crate::sharded_block_executor::test_utils;

#[test]
fn test_sharded_block_executor_no_conflict() {
    let num_shards = 8;
    let executor_shards = LocalExecutorShard::create_local_executor_shards(num_shards, Some(2));
    let sharded_block_executor = ShardedBlockExecutor::new(executor_shards);
    test_utils::test_sharded_block_executor_no_conflict(sharded_block_executor);
}

#[test]
// Sharded execution with cross shard conflict doesn't work for now because we don't have
// cross round dependency tracking yet.
fn test_sharded_block_executor_with_conflict_parallel() {
    let num_shards = 7;
    let executor_shards = LocalExecutorShard::create_local_executor_shards(num_shards, Some(4));
    let sharded_block_executor = ShardedBlockExecutor::new(executor_shards);
    test_utils::sharded_block_executor_with_conflict(sharded_block_executor, 4);
}

#[test]
fn test_sharded_block_executor_with_conflict_sequential() {
    let num_shards = 7;
    let executor_shards = LocalExecutorShard::create_local_executor_shards(num_shards, Some(1));
    let sharded_block_executor = ShardedBlockExecutor::new(executor_shards);
    test_utils::sharded_block_executor_with_conflict(sharded_block_executor, 1)
}

#[test]
fn test_sharded_block_executor_with_random_transfers_parallel() {
    test_utils::sharded_block_executor_with_random_transfers(4)
}

#[test]
fn test_sharded_block_executor_with_random_transfers_sequential() {
    test_utils::sharded_block_executor_with_random_transfers(1)
}
