pub mod error;
pub mod miner;
mod notify;
mod store;

use error::MinerError;
use std::sync::Mutex;
use store::db::DB;

use log::info;
use miner::entry::*;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref LCD_DB: Mutex<Option<DB>> = Mutex::new(None);
}

pub struct MinersLibConfig {
    pub app_path: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub feishu_bot: String,
    pub is_need_db: bool,
}

/// init lcd
pub fn init(config: &MinersLibConfig) {
    // init sqlite db
    if config.is_need_db {
        let mut db = LCD_DB.lock().unwrap();
        let db_inst = DB::new(&config.app_path).unwrap();
        *db = Some(db_inst);
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
