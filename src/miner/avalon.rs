use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::{fmt, time::Duration};

use super::entry::*;
use crate::error::MinerError;
//use curl::easy::Easy;
use log::info;
use regex::Regex;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AvalonWorkStatus {
    pub elapsed: i64,
    pub hash_real: f64,
    pub hash_avg: f64,
    pub temp: f64,
    pub tavg: String,
    pub work_status: String,
    pub work_mode: i32,
}

impl AvalonWorkStatus {
    pub fn is_same_work_mode(&self, account: &Account) -> bool {
        self.work_mode == if account.run_mode == "高功" { 1 } else { 0 }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AvalonPowerStatus {
    pub control_board_volt: f64,
    pub hash_board_volt: f64,
    pub amperage: f64,
    pub power: f64,
}

// Avalon config json struct
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AvalonConfig {
    #[serde(deserialize_with = "deserialize_mode")]
    pub mode: i32,
    pub pool1: String,
    pub worker1: String,
    pub passwd1: String,
    pub pool2: String,
    pub worker2: String,
    pub passwd2: String,
    pub pool3: String,
    pub worker3: String,
    pub passwd3: String,
}

fn deserialize_mode<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrInt;

    impl<'de> Visitor<'de> for StringOrInt {
        type Value = i32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<i32, E>
        where
            E: de::Error,
        {
            value.parse::<i32>().map_err(de::Error::custom)
        }

        fn visit_i64<E>(self, value: i64) -> Result<i32, E>
        where
            E: de::Error,
        {
            Ok(value as i32)
        }

        fn visit_u64<E>(self, value: u64) -> Result<i32, E>
        where
            E: de::Error,
        {
            if value <= i32::MAX as u64 {
                Ok(value as i32)
            } else {
                Err(de::Error::custom("u64 value was too large to fit in i32"))
            }
        }
    }

    deserializer.deserialize_any(StringOrInt)
}

/// Avalon miner
#[derive(Debug, Clone)]
pub struct AvalonMiner {}

impl MinerOperation for AvalonMiner {
    fn info(&self) -> MinerInfo {
        MinerInfo {
            name: "avalon".to_string(),
            detail: "Avalon is a miner".to_string(),
        }
    }

    fn detect<'a>(&self, _headers: Vec<String>, body: &'a str) -> Result<MinerType, MinerError> {
        // If body contains Avalon Device
        // direct string find
        if body.contains("Avalon Device") {
            Ok(MinerType::Avalon(AvalonMiner {}))
        } else {
            Err(MinerError::MinerNotSupportError)
        }
    }

    fn switch_account_if_diff(
        &self,
        ip: &str,
        account: &Account,
        is_force: bool,
    ) -> AsyncOpType<()> {
        let ip = ip.to_string();
        let account = account.clone();
        Box::pin(async move {
            // login --> get config --> update config --> reboot
            match switch_if_need(&ip, &account, is_force) {
                Ok(_) => Ok(()),
                Err(e) => {
                    info!("avalon switch account error: {:?}", e);
                    // try to ping
                    try_ping(&ip)?;
                    Ok(())
                }
            }
        })
    }

    fn query(&self, ip: &str, timeout_seconds: i64) -> Result<MachineInfo, MinerError> {
        let versio = tcp_query_version(&ip, timeout_seconds)?;
        // extract MODEL=xxx from version
        let re = Regex::new(r"MODEL=([^,]+),").unwrap();
        let machine_type = match re.captures(&versio) {
            Some(caps) => caps.get(1).unwrap().as_str().to_string(),
            None => "Avalon".to_string(),
        };

        let work = tcp_query_status(&ip, timeout_seconds)?;
        let pools = tcp_query_pool(&ip, timeout_seconds)?;
        let power_info = tcp_query_power(&ip, timeout_seconds)?;

        let temps = work.tavg.split(' ').collect::<Vec<&str>>();

        let elapsed_str = format!(
            "{}H {}M {}S",
            work.elapsed / 3600,
            (work.elapsed % 3600) / 60,
            work.elapsed % 60
        );

        Ok(MachineInfo {
            ip: ip.to_string(),
            elapsed: elapsed_str,
            hash_real: format!("{:.2} THS", work.hash_real / 1000.0),
            hash_avg: format!("{:.2} THS", work.hash_avg / 1000.0),
            machine_type: machine_type.clone(),
            temp: work.temp.to_string() + "/" + &work.tavg.replace(" ", "/"),
            fan: "0".to_string(),
            pool1: pools[0].url.clone().replace("stratum+tcp://", ""),
            worker1: pools[0].user.clone(),
            pool2: pools[1].url.clone().replace("stratum+tcp://", ""),
            worker2: pools[1].user.clone(),
            mode: if work.work_mode == 1 {
                "高功".to_string()
            } else {
                "普通".to_string()
            },
            pool_hash_avg: "N/A".to_string(),
            pool_hash_real: "N/A".to_string(),
            record: MachineRecord {
                id: 0,
                ip: ip.to_string(),
                machine_type,
                work_mode: work.work_mode,
                hash_real: work.hash_real,
                hash_avg: work.hash_avg,
                temp_0: temps[0].parse::<f64>().unwrap_or(0.0),
                temp_1: temps[1].parse::<f64>().unwrap_or(0.0),
                temp_2: temps[2].parse::<f64>().unwrap_or(0.0),
                power: power_info.power as i32,
                // current timestamp
                create_time: chrono::Local::now().timestamp(),
            },
        })
    }

    fn reboot(&self, ip: &str) -> Result<(), MinerError> {
        tcp_write_reboot(ip, 3)
    }

    fn config_pool(&self, ip: &str, pools: &Vec<PoolConfig>) -> Result<(), MinerError> {
        let ip_splited: Vec<&str> = ip.split('.').collect();
        let pool_prefix = "stratum+tcp://";

        let mut update_pools = pools.clone();
        for pool in update_pools.iter_mut() {
            pool.url = pool_prefix.to_string() + &pool.url;
            pool.user = pool.user.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
        }
        tcp_write_pool_config(ip, update_pools, 3)?;
        tcp_write_reboot(ip, 3)
    }

    fn config_mode(&self, ip: &str, mode: &str) -> Result<(), MinerError> {
        tcp_write_workmode(ip, if mode == "高功" { 1 } else { 0 }, 3)
    }

    fn config(&self, ip: &str, mode: &str, pools: &Vec<PoolConfig>) -> Result<(), MinerError> {
        let ip_splited: Vec<&str> = ip.split('.').collect();
        let pool_prefix = "stratum+tcp://";

        let mut update_pools = pools.clone();
        for pool in update_pools.iter_mut() {
            pool.url = pool_prefix.to_string() + &pool.url;
            pool.user = pool.user.clone() + ".a" + ip_splited[2] + "x" + ip_splited[3];
        }
        tcp_write_pool_config(ip, update_pools, 3)?;
        tcp_write_workmode(ip, if mode == "高功" { 1 } else { 0 }, 3)?;
        tcp_write_reboot(ip, 3)
    }
}

fn switch_if_need(ip: &str, account: &Account, is_force: bool) -> Result<(), MinerError> {
    let timeout = 3i64;
    let account_result = tcp_query_account(ip, timeout)?;
    let work = tcp_query_status(ip, timeout)?;
    //info!("avalon account result: {} {}", ip, account_result);
    let worker = account_result.split('.').next().unwrap();
    let config_worker = account.name.split('.').next().unwrap();

    if !is_force && worker == config_worker && work.is_same_work_mode(account) {
        info!("avalon end switch account no change: {}", ip);
        return Ok(());
    }

    let ip_splited: Vec<&str> = ip.split('.').collect();
    let user = account.name.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
    let act = Account {
        id: 1i32,
        name: user,
        password: account.password.clone(),
        pool1: account.pool1.clone(),
        pool2: account.pool2.clone(),
        pool3: account.pool3.clone(),
        run_mode: account.run_mode.clone(),
    };

    tcp_write_pool(ip, &act, timeout)?;
    tcp_write_workmode(ip, if account.run_mode == "高功" { 1 } else { 0 }, timeout)?;
    tcp_write_reboot(ip, timeout)?;
    info!("avalon end switch account: {}", ip);
    Ok(())
}

fn tcp_cmd(
    ip: &str,
    port: u16,
    cmd: &str,
    is_waiting_write: bool,
    timeout_seconds: i64,
) -> Result<String, MinerError> {
    let addr = format!("{}:{}", ip, port);
    let addrs = addr.to_socket_addrs()?.next().unwrap();
    let timeout_connect = Duration::from_secs(timeout_seconds as u64);
    let timeout_read_write = Duration::from_secs(timeout_seconds as u64);

    let mut stream = std::net::TcpStream::connect_timeout(&addrs, timeout_connect)?;
    stream.set_read_timeout(Some(timeout_read_write))?;
    stream.set_write_timeout(Some(timeout_read_write))?;
    stream.write_all(cmd.as_bytes())?;
    //info!("write done for cmd {}", cmd);

    if is_waiting_write {
        let mut buf = vec![0; 32768];
        let mut total_bytes_read = 0;
        let mut count = 0;

        loop {
            match stream.read(&mut buf[total_bytes_read..]) {
                Ok(n) => {
                    if n == 0 {
                        break;
                    }
                    total_bytes_read += n;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    count += 1;
                    //info!("avalon tcp_query WouldBlock: {}", count);
                    if count >= 3 {
                        break;
                    }
                    // Sleep for a while before trying to read again
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(e) => {
                    info!("avalon tcp_query error: {:?}", e);
                    return Err(e.into());
                }
            }
        }

        if total_bytes_read > 0 {
            let res = String::from_utf8(buf[..total_bytes_read].to_vec())?;
            //info!("avalon tcp_query result: {}", res);
            return Ok(res);
        }

        return Err(MinerError::TcpReadError);
    }

    return Ok("".to_string());
}

/// query version
pub fn tcp_query_version(ip: &str, timeout_seconds: i64) -> Result<String, MinerError> {
    tcp_cmd(ip, 4028, "version", true, timeout_seconds)
}

/// query pool
fn tcp_query_account(ip: &str, timeout_seconds: i64) -> Result<String, MinerError> {
    let pool = tcp_cmd(ip, 4028, "pools", true, timeout_seconds)?;
    //info!("avalon tcp_query_account result: {}", pool);
    // find first User=xxx, extract xxx
    let re = Regex::new(r"User=([^,]+),").unwrap();
    match re.captures(&pool) {
        Some(caps) => {
            let target = caps.get(1).unwrap().as_str();
            //info!("User target: {}", target);
            Ok(target.to_string())
        }
        None => Err(MinerError::ReadAvalonConfigError),
    }
}

fn tcp_query_pool(ip: &str, timeout_seconds: i64) -> Result<Vec<PoolConfig>, MinerError> {
    let res = tcp_cmd(ip, 4028, "pools", true, timeout_seconds)?;
    //info!("avalon tcp_query_pool result: {}", pool);
    // extract pool info
    let re = Regex::new(r"POOL=\d+,URL=([^,]+),.*?User=([^,]+),").unwrap();
    let mut pools = Vec::new();
    for cap in re.captures_iter(&res) {
        let pool = PoolConfig {
            url: cap.get(1).unwrap().as_str().to_string(),
            user: cap.get(2).unwrap().as_str().to_string(),
            password: "".to_string(),
        };
        pools.push(pool);
    }

    Ok(pools)
}

/// update pool
fn tcp_write_pool(ip: &str, pool: &Account, timeout_seconds: i64) -> Result<(), MinerError> {
    // ascset|0,setpool,root,root,2,stratum+tcp://btc.ss.poolin.com:443,cctrix.001,123
    let pool1 = format!(
        "ascset|0,setpool,root,root,0,{},{},{}",
        pool.pool1, pool.name, pool.password
    );

    let pool2 = format!(
        "ascset|0,setpool,root,root,1,{},{},{}",
        pool.pool2, pool.name, pool.password
    );

    let pool3 = format!(
        "ascset|0,setpool,root,root,2,{},{},{}",
        pool.pool3, pool.name, pool.password
    );

    tcp_cmd(ip, 4028, &pool1, true, timeout_seconds)?;
    tcp_cmd(ip, 4028, &pool2, true, timeout_seconds)?;
    tcp_cmd(ip, 4028, &pool3, true, timeout_seconds)?;

    Ok(())
}

fn tcp_write_pool_config(
    ip: &str,
    pools: Vec<PoolConfig>,
    timeout_seconds: i64,
) -> Result<(), MinerError> {
    for (i, pool) in pools.iter().enumerate() {
        let cmd = format!(
            "ascset|0,setpool,root,root,{},{},{},{}",
            i, pool.url, pool.user, pool.password
        );
        tcp_cmd(ip, 4028, &cmd, true, timeout_seconds)?;
    }

    Ok(())
}

fn tcp_write_workmode(ip: &str, mode: i32, timeout_seconds: i64) -> Result<(), MinerError> {
    // ascset|0,workmode,1
    let cmd = format!("ascset|0,workmode,{}", mode);
    tcp_cmd(ip, 4028, &cmd, true, timeout_seconds)?;
    Ok(())
}

fn tcp_query_status(ip: &str, timeout_seconds: i64) -> Result<AvalonWorkStatus, MinerError> {
    let res = tcp_cmd(ip, 4028, "estats", true, timeout_seconds)?;
    //info!("avalon tcp_query_status result: {}", res);
    let mut work: AvalonWorkStatus = AvalonWorkStatus::default();
    // SYSTEMSTATU[Work: In Work, Hash Board: 3 ] ... Elapsed[1697]
    let re = Regex::new(
        r"SYSTEMSTATU\[Work: (.*),.*Elapsed\[(\d+)\].*Temp\[(-?\d+)\].*GHSspd\[(\d+\.?\d*)\].**GHSavg\[(\d+\.?\d*)\].*MTavg\[(-?\d+ -?\d+ -?\d+)\].*WORKMODE\[(\d+)\]",
    )
    .unwrap();
    match re.captures(&res) {
        Some(caps) => {
            work.work_status = caps.get(1).map_or("", |m| m.as_str()).to_string();
            work.elapsed = caps
                .get(2)
                .map_or(0, |m| m.as_str().parse::<i64>().unwrap());
            work.temp = caps
                .get(3)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
            work.hash_real = caps
                .get(4)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
            work.hash_avg = caps
                .get(5)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
            work.tavg = caps.get(6).map_or("", |m| m.as_str()).to_string();
            work.work_mode = caps
                .get(7)
                .map_or(0, |m| m.as_str().parse::<i32>().unwrap());
        }
        None => return Err(MinerError::ReadAvalonConfigError),
    }

    Ok(work)
}

fn tcp_query_power(ip: &str, timeout_seconds: i64) -> Result<AvalonPowerStatus, MinerError> {
    let res = tcp_cmd(ip, 4028, "ascset|0,hashpower", true, timeout_seconds)?;
    let mut power = AvalonPowerStatus::default();
    // extract PS[0 1196 1284 230 2953 1284] from res
    let re = Regex::new(r"PS\[(\d+) (\d+) (\d+) (\d+) (\d+) (\d+)\]").unwrap();
    match re.captures(&res) {
        Some(caps) => {
            power.control_board_volt = caps
                .get(2)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
            power.hash_board_volt = caps
                .get(3)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
            power.amperage = caps
                .get(4)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
            power.power = caps
                .get(5)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap());
        }
        None => return Err(MinerError::ReadAvalonConfigError),
    }

    Ok(power)
}

/// reboot machine
fn tcp_write_reboot(ip: &str, timeout_seconds: i64) -> Result<(), MinerError> {
    tcp_cmd(ip, 4028, "ascset|0,reboot,0", false, timeout_seconds)?; // cgminer-api-restart
    Ok(())
}

fn try_ping(ip: &str) -> Result<bool, MinerError> {
    let addr = ip.parse().unwrap();
    let data = [1, 2, 3, 4]; // ping data
    let timeout = Duration::from_secs(1);
    let options = ping_rs::PingOptions {
        ttl: 128,
        dont_fragment: true,
    };
    let result = ping_rs::send_ping(&addr, timeout, &data, Some(&options));
    match result {
        Ok(_reply) => Ok(true),
        Err(_e) => Err(MinerError::PingFiledError),
    }
}

//test
#[cfg(test)]
mod tests {
    use tokio::runtime::Runtime;

    use super::*;

    lazy_static! {
        static ref SETUP: () = {
            env_logger::init();
        };
    }

    #[tokio::test]
    async fn avalon_test_get_config() {
        let _ = *SETUP;
        let ip = "192.168.187.170";
        // let mut easy = Easy::new();
        // let config = get_config(&mut easy, ip).unwrap().unwrap();
        // assert_eq!(config.mode, 1);
    }

    #[tokio::test]
    async fn avalon_test_update_config() {
        let _ = *SETUP;
        let ip = "192.168.189.162";
        let account = Account {
            id: 1i32,
            name: "sl002".to_string(),
            password: "1212".to_string(),
            pool1: "stratum+tcp://192.168.190.8:9011".to_string(),
            pool2: "stratum+tcp://192.168.190.9:9011".to_string(),
            pool3: "stratum+tcp://192.168.190.8:9011".to_string(),
            run_mode: "高功".to_string(),
        };

        let miner = AvalonMiner {};
        // let res = miner
        //     .switch_account_if_diff(ip.to_string(), account, true)
        //     .unwrap();
        // assert_eq!(res, ());

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let result = miner.switch_account_if_diff(ip, &account, true).await;
            match result {
                Ok(_) => println!("Switch account successful"),
                Err(e) => println!("Error switching account: {:?}", e),
            }
        });
    }

    // #[tokio::test]
    // async fn avalon_test_login() {
    //     env_logger::init();
    //     let ip = "192.168.187.167";
    //     let mut easy = Easy::new();
    //     let res = login(&mut easy, ip).unwrap();
    //     assert_eq!(res, ());
    // }

    #[tokio::test]
    async fn avalon_test_reboot() {
        let _ = *SETUP;
        let ip = "192.168.189.207";
        // let mut easy = Easy::new();
        // let res = reboot(&mut easy, ip).unwrap();
        // assert_eq!(res, ());
    }

    #[tokio::test]
    async fn avalon_test_query() {
        let _ = *SETUP;
        let ip = "192.168.189.207";
        let miner = AvalonMiner {};
        let info = miner.query(ip, 3).unwrap();
        info!("avalon info: {:?}", info);
    }

    #[test]
    fn avalon_tcp_query_version() {
        let _ = *SETUP;
        let ip = "192.168.187.186";
        let res = tcp_query_version(ip, 3).unwrap();
        info!("avalon tcp_query_version result: {}", res);
        assert!(res.contains("STATUS"));
    }

    #[test]
    fn avalon_tcp_cmd_reboot() {
        let _ = *SETUP;
        let ip = "192.168.189.213";
        let _res = tcp_write_reboot(ip, 3).unwrap();
        assert!(true);
    }

    #[test]
    fn avalon_tcp_query_account() {
        let _ = *SETUP;
        let ip = "192.168.189.212";
        let res = tcp_query_account(ip, 3).unwrap();
        info!("avalon tcp_query_account result: {}", res);
        assert!(true);
    }

    #[test]
    fn avalon_tcp_query_pool() {
        let _ = *SETUP;
        let ip = "192.168.189.212";
        let res = tcp_query_pool(ip, 3).unwrap();
        info!("avalon tcp_query_pool result: {:?}", res);
        assert!(true);
    }

    #[test]
    fn avalon_tcp_write_pool() {
        let _ = *SETUP;
        let ip = "192.168.187.186";
        let account = Account {
            id: 1i32,
            name: "sl002".to_string(),
            password: "1212".to_string(),
            pool1: "stratum+tcp://192.168.190.9:9011".to_string(),
            pool2: "stratum+tcp://192.168.190.8:9011".to_string(),
            pool3: "stratum+tcp://192.168.190.8:9011".to_string(),
            run_mode: "0".to_string(),
        };
        let res = tcp_write_pool(ip, &account, 3).unwrap();
        assert!(true);
    }

    #[test]
    fn avalon_tcp_query_status() {
        let _ = *SETUP;
        let ip = "192.168.188.22";
        let res = tcp_query_status(ip, 3).unwrap();
        info!("avalon tcp_query_status result: {:?}", res);
        assert!(true);
    }

    #[test]
    fn avalon_tcp_query_power() {
        let _ = *SETUP;
        let ip = "192.168.189.170";
        let res = tcp_query_power(ip, 3).unwrap();
        info!("avalon tcp_query_power result: {:?}", res);
        assert!(true);
    }
}
