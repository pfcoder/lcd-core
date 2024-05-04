pub mod error;
pub mod miner;
mod notify;
mod store;

use error::MinerError;

use log::info;
use miner::entry::*;

use crate::store::db;

#[macro_use]
extern crate lazy_static;

pub struct MinersLibConfig {
    pub app_path: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub feishu_bot: String,
    pub is_need_db: bool,
    pub db_keep_days: i64,
}

/// init lcd
pub fn init(config: &MinersLibConfig) {
    // init sqlite db
    if config.is_need_db {
        db::init(&config.app_path, config.db_keep_days);
    }

    notify::feishu::init(
        &config.feishu_app_id,
        &config.feishu_app_secret,
        &config.feishu_bot,
    );

    info!("lcd initialized.");
}

/// switch miner config as config
pub async fn switch_if_need(
    runtime: tokio::runtime::Handle,
    excel: &str,
    sheets: Vec<&str>,
    account_time_sheet: &str,
    perf_time_sheet: &str,
    pool_sheet: &str,
) -> Result<(), MinerError> {
    miner::entry::switch_if_need(
        runtime,
        excel,
        sheets,
        account_time_sheet,
        perf_time_sheet,
        pool_sheet,
    )
    .await
}

/// scan
pub async fn scan(
    runtime: tokio::runtime::Handle,
    ip: &str,
    offset: i32,
    count: i32,
) -> Result<Vec<MachineInfo>, String> {
    info!("scan ip: {}", ip);
    miner::entry::scan(runtime, ip, offset, count).await
}

/// batch reboot
pub async fn reboot(runtime: tokio::runtime::Handle, ips: Vec<String>) -> Result<(), String> {
    info!("reboot ips: {:?}", ips);
    miner::entry::reboot_batch(runtime, ips).await
}

/// batch config
pub async fn config(
    runtime: tokio::runtime::Handle,
    ips: Vec<String>,
    account: Vec<PoolConfig>,
) -> Result<i64, String> {
    info!("config ips: {:?}", ips);
    miner::entry::config_batch(runtime, ips, account).await
}

/// watching
pub async fn watching(
    runtime: tokio::runtime::Handle,
    ips: Vec<String>,
) -> Result<Vec<MachineInfo>, String> {
    miner::entry::watching(runtime, ips).await
}

/// query machine records
pub fn query_machine_records_by_time(
    ip: String,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<MachineRecord>, String> {
    match db::query_records_by_time(ip, start_time, end_time) {
        Ok(records) => Ok(records),
        Err(e) => Err(e.to_string()),
    }
}

/// clear records before time
pub fn clear_records_before_time(time: i64) -> Result<(), String> {
    match db::clear_records_before_time(time) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
