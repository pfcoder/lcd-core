use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Mutex;
use std::{collections::BTreeMap, time::Duration};

use chrono::NaiveTime;
use curl::easy::Easy;
use log::info;
use serde::{Deserialize, Serialize};

use crate::{error::MinerError, notify::feishu};

use super::{ant::*, avalon::*, bluestar::*};

lazy_static! {
    static ref ERR_MAP: Mutex<HashMap<String, i32>> = Mutex::new(HashMap::new());
}

#[derive(Debug, Clone)]
pub enum MinerType {
    Ant(AntMiner),
    Avalon(AvalonMiner),
    BlueStar(BlueStarMiner),
}

// from str to enum
impl From<&str> for MinerType {
    fn from(s: &str) -> Self {
        match s {
            "ant" => MinerType::Ant(AntMiner {}),
            "avalon" => MinerType::Avalon(AvalonMiner {}),
            "bluestar" => MinerType::BlueStar(BlueStarMiner {}),
            _ => panic!("MinerType not support"),
        }
    }
}

#[derive(Debug, Clone)]
struct TimeConfig {
    pub start: NaiveTime,     // 00:00:00
    pub end: NaiveTime,       // 00:00:00
    pub account_type: String, // main or switch or perf mode(高功/普通)
}

impl TimeConfig {
    pub fn now_account(&self) -> Option<&str> {
        let now_time = chrono::Local::now().time();
        if (self.start < self.end && now_time >= self.start && now_time <= self.end)
            || (self.start > self.end && (now_time >= self.start || now_time <= self.end))
        {
            return Some(&self.account_type);
        }
        None
    }

    pub fn from_str(start: &str, end: &str, account_type: &str) -> Result<TimeConfig, MinerError> {
        let start = NaiveTime::parse_from_str(start, "%H:%M:%S")?;
        let end = NaiveTime::parse_from_str(end, "%H:%M:%S")?;
        Ok(TimeConfig {
            start,
            end,
            account_type: account_type.to_string(),
        })
    }

    pub fn now_perf(&self) -> Option<&str> {
        let now_time = chrono::Local::now().time();
        if (self.start < self.end && now_time >= self.start && now_time <= self.end)
            || (self.start > self.end && (now_time >= self.start || now_time <= self.end))
        {
            return Some(&self.account_type);
        }
        None
    }
}

// String type enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MinerStatus {
    Online,
    Offline,
    // Error,
}

impl From<&str> for MinerStatus {
    fn from(s: &str) -> Self {
        match s {
            "上线" => MinerStatus::Online,
            _ => MinerStatus::Offline,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub id: i32,
    pub account_id: i32,
    pub ip: String,
    pub name: String, // ant, avalon, bluestar
    pub status: MinerStatus,
    pub account: Account,
    pub switch_account: Option<Account>,
    pub addition_info: String,
    pub run_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub ip: String,
    pub machine_type: String,
    pub hash_real: String,
    pub hash_avg: String,
    pub temp: String,
    pub fan: String,
    pub elapsed: String,
    pub pool1: String,
    pub worker1: String,
    pub pool2: String,
    pub worker2: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i32,
    pub name: String,
    pub password: String,
    pub pool1: String,
    pub pool2: String,
    pub pool3: String,
    pub run_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct ErrorRecord {
    pub machine: Machine,
    pub error: String,
}

pub type AsyncOpType<T> = Pin<Box<dyn std::future::Future<Output = Result<T, MinerError>> + Send>>;

pub trait MinerOperation {
    fn info(&self) -> MinerInfo;
    fn detect(&self, headers: Vec<String>, body: &str) -> Result<MinerType, MinerError>;
    fn query(&self, ip: String) -> Result<MachineInfo, MinerError>;
    fn switch_account_if_diff(
        &self,
        ip: String,
        account: Account,
        is_force: bool,
    ) -> AsyncOpType<()>;
    fn reboot(&self, ip: String) -> Result<(), MinerError>;
    fn config_pool(&self, ip: String, pools: Vec<PoolConfig>) -> Result<(), MinerError>;
}

#[derive(Debug, Clone)]
pub struct MinerInfo {
    pub name: String,
    pub detail: String,
}

/// supported miner array
pub const MINERS: [MinerType; 3] = [
    MinerType::Ant(AntMiner {}),
    MinerType::Avalon(AvalonMiner {}),
    MinerType::BlueStar(BlueStarMiner {}),
];

impl MinerOperation for MinerType {
    fn info(&self) -> MinerInfo {
        match self {
            MinerType::Ant(miner) => miner.info(),
            MinerType::Avalon(miner) => miner.info(),
            MinerType::BlueStar(miner) => miner.info(),
        }
    }

    fn detect(&self, headers: Vec<String>, body: &str) -> Result<MinerType, MinerError> {
        match self {
            MinerType::Ant(miner) => miner.detect(headers, body),
            MinerType::Avalon(miner) => miner.detect(headers, body),
            MinerType::BlueStar(miner) => miner.detect(headers, body),
        }
    }

    fn switch_account_if_diff(
        &self,
        ip: String,
        account: Account,
        is_force: bool,
    ) -> AsyncOpType<()> {
        match self {
            MinerType::Ant(miner) => miner.switch_account_if_diff(ip, account, is_force),
            MinerType::Avalon(miner) => miner.switch_account_if_diff(ip, account, is_force),
            MinerType::BlueStar(miner) => miner.switch_account_if_diff(ip, account, is_force),
        }
    }

    fn query(&self, ip: String) -> Result<MachineInfo, MinerError> {
        match self {
            MinerType::Ant(miner) => miner.query(ip),
            MinerType::Avalon(miner) => miner.query(ip),
            MinerType::BlueStar(miner) => miner.query(ip),
        }
    }

    fn reboot(&self, ip: String) -> Result<(), MinerError> {
        match self {
            MinerType::Ant(miner) => miner.reboot(ip),
            MinerType::Avalon(miner) => miner.reboot(ip),
            MinerType::BlueStar(miner) => miner.reboot(ip),
        }
    }

    fn config_pool(&self, ip: String, pools: Vec<PoolConfig>) -> Result<(), MinerError> {
        match self {
            MinerType::Ant(miner) => miner.config_pool(ip, pools),
            MinerType::Avalon(miner) => miner.config_pool(ip, pools),
            MinerType::BlueStar(miner) => miner.config_pool(ip, pools),
        }
    }
}

fn find_miner(ip: &str) -> Result<MinerType, MinerError> {
    info!("start detect: {}", ip);
    let mut easy = Easy::new();
    easy.url(&ip)?;
    // timeout 5s
    easy.timeout(Duration::from_secs(3))?;
    let mut headers = Vec::new();
    let mut data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            data.extend_from_slice(new_data);
            Ok(new_data.len())
        })?;
        transfer.header_function(|header| {
            headers.push(String::from_utf8(header.to_vec()).unwrap());
            true
        })?;
        transfer.perform()?;
    }

    easy.perform()?;

    let body = String::from_utf8(data).unwrap();

    for miner in MINERS.iter() {
        if let Ok(miner_inst) = miner.detect(headers.clone(), &body) {
            info!("detect miner: {} {}", ip, miner.info().name);
            // perform query
            return Ok(miner_inst);
        }
    }

    Err(MinerError::MinerNotSupportError)
}

pub fn scan_miner_detail(ip: String) -> AsyncOpType<MachineInfo> {
    Box::pin(async move {
        let miner = find_miner(&ip)?;
        miner.query(ip)
    })
}

fn scan_reboot(ip: String) -> Result<(), MinerError> {
    info!("try to reboot: {}", ip);
    let miner = find_miner(&ip)?;
    miner.reboot(ip)
}

pub async fn load_machines_from_feishu(
    excel: &str,
    sheets: Vec<&str>,
    pools_map: &HashMap<String, Vec<String>>,
) -> Result<BTreeMap<String, Vec<Machine>>, MinerError> {
    let mut machine_map: BTreeMap<String, Vec<Machine>> = BTreeMap::new();
    // go through sheets to load
    for sheet in sheets.iter() {
        let json_result = feishu::query_sheet(excel, sheet).await?;
        // query data.valueRange.values
        let values = json_result["data"]["valueRange"]["values"]
            .as_array()
            .ok_or(MinerError::FeishuParserJsonError)?;
        // ignore first row
        for row in values.iter().skip(1) {
            let miner_type;
            let account;
            let pool;
            let switch_account_name: Option<String>;
            let switch_pool: Option<String>;
            let switch_account: Option<Account>;
            match row[0].as_str() {
                Some("avalon") => {
                    miner_type = MinerType::Avalon(AvalonMiner {});
                }
                Some("ant") => {
                    miner_type = MinerType::Ant(AntMiner {});
                }
                Some("bluestar") => {
                    miner_type = MinerType::BlueStar(BlueStarMiner {});
                }
                _ => continue,
            }
            let ip = match row[3].as_str() {
                Some(ip) => ip,
                None => continue,
            };
            //let name = row[5].as_str().ok_or(MinerError::FeishuParserJsonError)?;
            let status: MinerStatus = match row[4].as_str() {
                Some(sts) => sts.into(),
                _ => continue,
            };
            let account_name = match row[8].as_str() {
                Some(account_name) => account_name,
                None => continue,
            };

            match row[9].as_str() {
                Some(main_pool) => {
                    // ignore empty string
                    if main_pool.len() > 0 {
                        pool = main_pool.to_string();
                    } else {
                        continue;
                    }
                }
                _ => {
                    // ignore other pool
                    continue;
                }
            }

            match row[10].as_str() {
                Some(acct) => {
                    // ignore empty string
                    if acct.len() > 0 {
                        switch_account_name = Some(acct.to_string());
                    } else {
                        switch_account_name = None;
                    }
                }
                _ => {
                    switch_account_name = None;
                }
            }

            match row[11].as_str() {
                Some(pool) => {
                    // ignore empty string
                    if pool.len() > 0 {
                        switch_pool = Some(pool.to_string());
                    } else {
                        switch_pool = None;
                    }
                }
                _ => {
                    switch_pool = None;
                }
            }

            let main_account_working_mode = match row[12].as_str() {
                Some(mode) => mode.to_string(),
                None => "".to_string(),
            };

            let switch_account_working_mode = match row[13].as_str() {
                Some(mode) => mode.to_string(),
                None => "".to_string(),
            };

            let pools = get_pool(&pool, &miner_type.info().name, pools_map);

            if pools.len() != 3 {
                continue;
            }

            account = Account {
                id: 0,
                name: account_name.to_string(),
                password: "auto".to_string(),
                pool1: pools[0].to_owned(),
                pool2: pools[1].to_owned(),
                pool3: pools[2].to_owned(),
                run_mode: main_account_working_mode,
            };

            switch_account = match switch_account_name {
                Some(name) => {
                    let pools = get_pool(&switch_pool.unwrap(), &miner_type.info().name, pools_map);
                    if pools.len() != 3 {
                        continue;
                    }
                    Some(Account {
                        id: 0,
                        name: name.to_string(),
                        password: "auto".to_string(),
                        pool1: pools[0].to_owned(),
                        pool2: pools[1].to_owned(),
                        pool3: pools[2].to_owned(),
                        run_mode: switch_account_working_mode,
                    })
                }
                None => None,
            };

            let addition_info = match row[14].as_str() {
                Some(info) => info.to_string(),
                None => "".to_string(),
            };

            let position = match row[2].as_str() {
                Some(pos) => pos.to_string(),
                None => "".to_string(),
            };

            let machine = Machine {
                id: 0,
                account_id: 0,
                ip: ip.to_string(),
                name: miner_type.info().name.clone(),
                status: status,
                account: account,
                switch_account: switch_account,
                run_mode: "".to_string(),
                addition_info: format!("{} {}", position, addition_info),
            };

            // put into map
            let machines = machine_map.entry(miner_type.info().name).or_insert(vec![]);
            machines.push(machine);
        }
    }

    Ok(machine_map)
}

pub async fn get_pools_from_feishu(
    excel: &str,
    sheet: &str,
) -> Result<HashMap<String, Vec<String>>, MinerError> {
    let json_result = feishu::query_sheet(excel, sheet).await?;

    let values = json_result["data"]["valueRange"]["values"]
        .as_array()
        .ok_or(MinerError::FeishuParserJsonError)?;

    let mut pools_map: HashMap<String, Vec<String>> = HashMap::new();

    for row in values.iter() {
        let pool_type = row[0].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let pool1 = row[1].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let pool2 = row[2].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let pool3 = row[3].as_str().ok_or(MinerError::FeishuParserJsonError)?;

        pools_map.insert(
            pool_type.to_string(),
            vec![pool1.to_string(), pool2.to_string(), pool3.to_string()],
        );
    }

    Ok(pools_map)
}

pub async fn get_perf_time_from_feishu(excel: &str, sheet: &str) -> Result<String, MinerError> {
    let json_result = feishu::query_sheet(excel, sheet).await?;

    let values = json_result["data"]["valueRange"]["values"]
        .as_array()
        .ok_or(MinerError::FeishuParserJsonError)?;

    for row in values.iter().skip(1) {
        let start = row[1].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let end = row[2].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let account_type = row[0].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let time_config = TimeConfig::from_str(start, end, account_type)?;
        if let Some(perf) = time_config.now_perf() {
            return Ok(perf.to_string());
        }
    }

    // default as 普通
    Ok("普通".to_string())
}

pub async fn get_now_account_type_from_feishu(
    excel: &str,
    sheet: &str,
) -> Result<String, MinerError> {
    let json_result = feishu::query_sheet(excel, sheet).await?;

    let values = json_result["data"]["valueRange"]["values"]
        .as_array()
        .ok_or(MinerError::FeishuParserJsonError)?;

    //let mut time_configs: TimeConfigs = vec![];
    for row in values.iter().skip(1) {
        let start = row[1].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let end = row[2].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        let account_type = row[0].as_str().ok_or(MinerError::FeishuParserJsonError)?;
        //time_configs.push(TimeConfig::from_str(start, end, account_type)?);
        info!(
            "start: {}, end: {}, account_type: {}",
            start, end, account_type
        );

        let time_config = TimeConfig::from_str(start, end, account_type)?;
        if let Some(account) = time_config.now_account() {
            return Ok(account.to_string());
        }
    }

    Err(MinerError::FeishuParserJsonError)
}

pub async fn switch_if_need(
    runtime: tokio::runtime::Handle,
    excel: &str,
    sheets: Vec<&str>,
    account_time_sheet: &str,
    perf_time_sheet: &str,
    pool_sheet: &str,
) -> Result<(), MinerError> {
    info!("start switch action");
    let account_type = get_now_account_type_from_feishu(excel, account_time_sheet).await?;
    let perf_mode = get_perf_time_from_feishu(excel, perf_time_sheet).await?;
    let pools_map = get_pools_from_feishu(excel, pool_sheet).await?;
    let machine_map = load_machines_from_feishu(excel, sheets, &pools_map).await?;
    let mut handles = Vec::new();
    let mut process_machines = vec![];

    for (miner_type, machines) in machine_map.iter() {
        for machine in machines {
            if machine.switch_account.is_some() && machine.status == MinerStatus::Online {
                // switch account
                let mut switch_account = if account_type == "main" {
                    machine.account.clone()
                } else {
                    machine.switch_account.clone().unwrap()
                };

                // check switch_account run_mode, if be “高功", the perf also should be "高功", then we set
                if switch_account.run_mode == "高功" && perf_mode == "高功" {
                    switch_account.run_mode = "高功".to_string();
                } else {
                    switch_account.run_mode = "普通".to_string();
                }

                let ip = machine.ip.clone();
                let miner: MinerType = miner_type.as_str().into();
                handles.push(runtime.spawn(miner.switch_account_if_diff(
                    ip,
                    switch_account,
                    false,
                )));

                process_machines.push(machine);
            }
        }
    }

    info!("switch action len: {:?}", handles.len());
    let result = futures::future::join_all(handles).await;
    info!("switch result len: {:?}", result.len());

    let mut error_ips: Vec<String> = vec![];
    let mut result_iter = result.iter();

    for machine in process_machines {
        match result_iter.next() {
            Some(res) => match res {
                Ok(action_result) => match action_result {
                    Ok(_) => {
                        //info!("switch success: {}", &machine.ip);
                    }
                    Err(e) => {
                        info!("switch failed: {} error: {:?}", &machine.ip, e);
                        error_ips.push(format!("[{}-{}]", &machine.ip, &machine.addition_info));
                    }
                },
                Err(e) => {
                    info!("join switch failed: {}, error: {:?}", &machine.ip, e);
                    error_ips.push(format!("[{}-{}] ", &machine.ip, &machine.addition_info));
                }
            },
            None => {
                info!("failed: {}", &machine.ip);
                error_ips.push(format!("[{}-{}] ", &machine.ip, &machine.addition_info));
            }
        }
    }

    if error_ips.len() > 0 {
        // check ERR_MAP, count when matched count > 3, notify
        let mut err_map = ERR_MAP.lock().unwrap();
        let mut selected_ips = vec![];
        for ip in error_ips.iter() {
            let count = err_map.entry(ip.to_string()).or_insert(0);
            *count += 1;
            if *count >= 3 {
                *count = 0;
                selected_ips.push(ip.to_string());
            }
        }

        if selected_ips.len() > 0 {
            let mut msg = format!(
                "{} 访问故障: ",
                chrono::Local::now().format("%H:%M:%S").to_string()
            );
            for ip in selected_ips.iter() {
                msg.push_str(ip);
            }
            info!("{}", msg);
            feishu::notify(&msg).await;
        }
    }

    info!("end switch action");
    Ok(())
}

fn get_pool(
    pool_type: &str,
    miner_type: &str,
    pools_map: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let prefix = match miner_type {
        "avalon" => "stratum+tcp://",
        _ => "",
    };

    match pools_map.get(pool_type) {
        Some(pools) => {
            let mut result = vec![];
            for pool in pools.iter() {
                result.push(prefix.to_owned() + pool);
            }
            result
        }
        None => vec![],
    }
}

/// Scan specified ip rand and update db
pub async fn scan(
    runtime: tokio::runtime::Handle,
    ip_demo: &str,
    offset: i32,
    count: i32,
) -> Result<Vec<MachineInfo>, String> {
    let ip_prefix = ip_demo.split('.').take(3).collect::<Vec<&str>>().join(".");
    info!(
        "scan_and_update_db ip_prefix: {} {} {}",
        ip_prefix, offset, count
    );
    // go through 1 - 255 with tokio handles
    let mut handles = vec![];
    for i in offset..(offset + count) {
        let ip = format!("{}.{}", ip_prefix, i);
        handles.push(runtime.spawn(async move { scan_miner_detail(ip).await }));
    }

    let result = futures::future::join_all(handles).await;

    // info!("scan_and_update_db result: {:?}", result);
    // fiter out Err from result
    let mut machines = vec![];
    for res in result {
        match res {
            Ok(Ok(machine)) => {
                machines.push(machine);
            }
            Ok(Err(e)) => {
                info!("scan_and_update_db error: {:?}", e);
            }
            Err(e) => {
                info!("scan_and_update_db join error: {:?}", e);
            }
        }
    }

    Ok(machines)
}

pub async fn watching(
    runtime: tokio::runtime::Handle,
    ips: Vec<String>,
) -> Result<Vec<MachineInfo>, String> {
    info!("watching ips: {:?}", ips);
    let mut handles = vec![];
    for ip in ips {
        handles.push(runtime.spawn(async move { scan_miner_detail(ip).await }));
    }

    let result = futures::future::join_all(handles).await;

    let mut machines = vec![];
    for res in result {
        match res {
            Ok(Ok(machine)) => {
                machines.push(machine);
            }
            Ok(Err(e)) => {
                info!("scan_and_update_db error: {:?}", e);
            }
            Err(e) => {
                info!("scan_and_update_db join error: {:?}", e);
            }
        }
    }

    Ok(machines)
}

pub async fn reboot_batch(runtime: tokio::runtime::Handle, ips: Vec<String>) -> Result<(), String> {
    let mut handles = vec![];
    for ip in ips {
        handles.push(runtime.spawn(async move { scan_reboot(ip) }));
    }

    let _result = futures::future::join_all(handles).await;

    Ok(())
}

pub async fn config_batch(
    runtime: tokio::runtime::Handle,
    ips: Vec<String>,
    pools: Vec<PoolConfig>,
) -> Result<i64, String> {
    let mut handles = vec![];
    for ip in ips {
        let act = pools.clone();
        handles.push(runtime.spawn(async move {
            let miner = find_miner(&ip)?;
            miner.config_pool(ip, act.clone())
        }));
    }

    let result = futures::future::join_all(handles).await;
    let mut count = 0;
    for res in result {
        match res {
            Ok(Ok(())) => {
                count += 1;
            }
            Ok(Err(e)) => {
                info!("scan_and_update_db error: {:?}", e);
            }
            Err(e) => {
                info!("scan_and_update_db join error: {:?}", e);
            }
        }
    }

    Ok(count)
}

//test
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    lazy_static! {
        static ref SETUP: () = {
            env_logger::init();
            let cli_id = std::env::var("CLIENT_ID").expect("CLIENT_ID is not set in env");
            let secret = std::env::var("SECRET").expect("SECRET is not set in env");
            let bot = std::env::var("BOT").expect("BOT is not set in env");
            feishu::init(&cli_id, &secret, &bot);
        };

        static ref TEST_RUNTIME: Runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(100) // 设置工作线程的数量
            .enable_all()
            .build()
            .unwrap();
    }

    #[tokio::test]
    async fn test_now_account() {
        let _ = &*SETUP;
        let account = get_now_account_type_from_feishu("PwjYsZoefh6rXZt3mIucC9XmnZb", "hoH6Gm")
            .await
            .unwrap();
        assert_eq!(account, "main");
    }

    #[tokio::test]
    async fn test_pools_map() {
        let _ = &*SETUP;
        let pools_map = get_pools_from_feishu("PwjYsZoefh6rXZt3mIucC9XmnZb", "IHJgN0")
            .await
            .unwrap();
        info!("pools_map: {:?}", pools_map);
        let pools = get_pool("鱼池", "avalon", &pools_map);
        info!("pools yu: {:?}", pools);
        assert_eq!(pools.len(), 3);
        let pools = get_pool("币印", "ant", &pools_map);
        info!("pools bi: {:?}", pools);
        assert_eq!(pools.len(), 3);
    }

    #[tokio::test]
    async fn test_auto_switch() {
        let _ = &*SETUP;

        switch_if_need(
            TEST_RUNTIME.handle().clone(),
            "PwjYsZoefh6rXZt3mIucC9XmnZb",
            vec!["ftMgRx"],
            "hoH6Gm",
            "u9zVVA",
            "IHJgN0",
        )
        .await
        .unwrap();
        assert!(true);
    }

    #[tokio::test]
    async fn test_scan_and_update_db() {
        let _ = &*SETUP;

        scan(TEST_RUNTIME.handle().clone(), "192.168.187.1", 0, 255)
            .await
            .unwrap();
        assert!(true);
    }
}
