// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0
use tracing::{info,debug,warn,error};
use anyhow::Result;
use move_core_types::{account_address::AccountAddress, effects::ChangeSet};
use move_stdlib::natives::{all_natives, nursery_natives, GasParameters, NurseryGasParameters};

fn main() -> Result<()> {
    // 初始化带颜色的控制台日志
    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .with_target(false)  // 不显示目标
    .with_file(true)     // 显示文件
    .with_line_number(true)  // 显示行号
    .init();

    info!("INFO 等级的日志");
    debug!("DEBUG 等级的日志");
    warn!("WARN 等级的日志");
    error!("ERROR 等级的日志");


    let cost_table = &move_vm_test_utils::gas_schedule::INITIAL_COST_SCHEDULE;
    let addr = AccountAddress::from_hex_literal("0x1").unwrap();
    let natives = all_natives(addr, GasParameters::zeros())
        .into_iter()
        .chain(nursery_natives(addr, NurseryGasParameters::zeros()))
        .collect();
    info!("开始运行 Move Cli");
    move_cli::move_cli(natives, ChangeSet::new(), cost_table)
}
