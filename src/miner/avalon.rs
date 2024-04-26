use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::{fmt, time::Duration};

use super::entry::*;
use crate::error::MinerError;
use curl::easy::{Easy, List};
use log::info;
use regex::Regex;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

const INFO_URL: &str = "http://{}/updatecgconf.cgi?num=";
const CONFIG_UPDATE_URL: &str = "http://{}/cgconf.cgi";
//const LOGIN_URL: &str = "http://{}/login.cgi";
const REBOOT_URL: &str = "http://{}/reboot.cgi";
const STATUS_URL: &str = "http://{}/get_home.cgi";
const MINER_TYPE_URL: &str = "http://{}/get_minerinfo.cgi";

// Avalon config json struct
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl AvalonConfig {
    pub fn to_form_string(&self) -> Result<String, MinerError> {
        serde_urlencoded::to_string(self).map_err(|err| MinerError::from(err))
    }

    pub fn apply_account(&mut self, account: &Account, ip: &str) {
        let ip_splited: Vec<&str> = ip.split('.').collect();
        let user = account.name.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
        self.pool1 = account.pool1.clone();
        self.worker1 = user.clone();
        self.passwd1 = account.password.clone();
        self.pool2 = account.pool2.clone();
        self.worker2 = user.clone();
        self.passwd2 = account.password.clone();
        self.pool3 = account.pool3.clone();
        self.worker3 = user.clone();
        self.passwd3 = account.password.clone();
        self.mode = if account.run_mode == "高功" { 1 } else { 0 };
    }

    pub fn is_same_account(&self, account: &Account) -> bool {
        // check string before .
        let worker1_splited: Vec<&str> = self.worker1.split('.').collect();
        let account_name_splited: Vec<&str> = account.name.split('.').collect();
        worker1_splited[0] == account_name_splited[0]
    }

    pub fn is_same_mode(&self, account: &Account) -> bool {
        self.mode == if account.run_mode == "高功" { 1 } else { 0 }
    }

    pub fn apply_pool_config(&mut self, pools: Vec<PoolConfig>, ip: &str) {
        let ip_splited: Vec<&str> = ip.split('.').collect();
        let pool_prefix = "stratum+tcp://";
        if pools.len() > 0 {
            self.pool1 = format!("{}{}", pool_prefix, pools[0].url);
            self.worker1 = pools[0].user.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
            self.passwd1 = pools[0].password.clone();
        }
        if pools.len() > 1 {
            self.pool2 = format!("{}{}", pool_prefix, pools[1].url);
            self.worker2 = pools[1].user.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
            self.passwd2 = pools[1].password.clone();
        }
        if pools.len() > 2 {
            self.pool3 = format!("{}{}", pool_prefix, pools[2].url);
            self.worker3 = pools[2].user.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
            self.passwd3 = pools[2].password.clone();
        }
    }
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
        ip: String,
        account: Account,
        is_force: bool,
    ) -> AsyncOpType<()> {
        Box::pin(async move {
            // login --> get config --> update config --> reboot
            info!("avalon start switch account: {}", ip);
            let mut easy = Easy::new();
            easy.timeout(Duration::from_secs(5))?;
            //home(&mut easy, &ip)?;
            //login(&mut easy, &ip)?;
            // if fail to get config, try to reboot
            // try to read current account through tcp
            let account_result = tcp_query_account(&ip)?;
            info!("avalon account result: {} {}", ip, account_result);
            let worker = account_result.split('.').next().unwrap();
            let config_worker = account.name.split('.').next().unwrap();

            let config_op = get_config(&mut easy, &ip).or_else(|_| {
                // try to read pools through tcp
                if worker != config_worker {
                    // reboot
                    // info!(
                    //     "avalon account not match and can not open web, reboot: {} {} {}",
                    //     ip, worker, config_worker
                    // );
                    tcp_write_reboot(&ip)?;
                    // return Err(MinerError::ReadAvalonConfigError);
                } else {
                    // worker is match, and can read from tcp approve machine live, do nothing, return Ok
                    // info!(
                    //     "avalon account match and can not open web, do nothing: {} {} {}",
                    //     ip, worker, config_worker
                    // );
                }

                // although web access error, but tcp is ok
                return Ok::<Option<AvalonConfig>, MinerError>(None);
            })?;
            let mut config;
            if let Some(c) = config_op {
                config = c;
            } else {
                info!("avalon end switch account config read fail: {}", ip);
                return Ok(());
            }

            if !is_force
                && worker == config_worker
                && config.is_same_account(&account)
                && config.is_same_mode(&account)
            {
                // info!(
                //     "avalon account and mode not changed: {} current_account:{} switch_account:{}, current_mode: {}, switch_mode: {}",
                //     ip, worker, account.name, config.mode, account.run_mode
                // );
                info!("avalon end switch account no change: {}", ip);
                return Ok(());
            }
            config.apply_account(&account, &ip);
            update_miner_config(&mut easy, &ip, &config)?;
            reboot(&mut easy, &ip)?;
            info!("avalon end switch account: {}", ip);
            Ok(())
        })
    }

    fn query(&self, ip: String) -> Result<MachineInfo, MinerError> {
        let machine_json = query_machine(&ip)?;
        let miner_json = query_miner_type(&ip)?;
        let conf = get_config(&mut Easy::new(), &ip)?.unwrap();

        let machine_type = miner_json["hwtype"].as_str().unwrap_or("unknown");
        let elapsed = machine_json["elapsed"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let hash_real = machine_json["hash_5m"]
            .as_str()
            .unwrap_or("0")
            .parse::<f64>()
            .unwrap_or(0.0);
        let hash_avg = machine_json["av"]
            .as_str()
            .unwrap_or("0")
            .parse::<f64>()
            .unwrap_or(0.0);
        let temp0 = machine_json["temperature"].as_str().unwrap_or("0");
        let temp1 = machine_json["MTavg1"].as_str().unwrap_or("0");
        let temp2 = machine_json["MTavg2"].as_str().unwrap_or("0");
        let temp3 = machine_json["MTavg3"].as_str().unwrap_or("0");
        // elapsed is seconds, convert to H:M:S
        let elapsed_str = format!(
            "{}H {}M {}S",
            elapsed / 3600,
            (elapsed % 3600) / 60,
            elapsed % 60
        );

        let mut pool1 = conf.pool1.clone();
        let mut pool2 = conf.pool2.clone();
        if pool1.starts_with("stratum+tcp://") {
            pool1.drain(..14);
        }
        if pool2.starts_with("stratum+tcp://") {
            pool2.drain(..14);
        }

        Ok(MachineInfo {
            ip,
            elapsed: elapsed_str,
            hash_real: format!("{:.2} THS", hash_real),
            hash_avg: format!("{:.2} THS", hash_avg),
            machine_type: machine_type.to_string(),
            temp: format!("{}/{}/{}/{}", temp0, temp1, temp2, temp3),
            fan: "0".to_string(),
            // remote "stratum+tcp://" prefix
            pool1,
            worker1: conf.worker1.clone(),
            pool2,
            worker2: conf.worker2.clone(),
        })
    }

    fn reboot(&self, ip: String) -> Result<(), MinerError> {
        Ok(reboot(&mut Easy::new(), &ip)?)
    }

    fn config_pool(&self, ip: String, pools: Vec<PoolConfig>) -> Result<(), MinerError> {
        let mut easy = Easy::new();
        let mut conf = get_config(&mut easy, &ip)?.unwrap();
        conf.apply_pool_config(pools, &ip);
        update_miner_config(&mut easy, &ip, &conf)?;
        reboot(&mut easy, &ip)
    }
}

// fn home(easy: &mut Easy, ip: &str) -> Result<(), MinerError> {
//     let url = format!("http://{}/", ip);
//     info!("avalon home url: {}", url);

//     easy.url(&url)?;
//     easy.post(false)?;
//     match easy.perform() {
//         Ok(_) => (),
//         Err(e) => {
//             info!("avalon home error: {:?}", e);
//             // try to reboot
//             reboot(easy, ip)?;
//         }
//     }

//     info!("avalon home status: {:?}", easy.response_code()?);

//     Ok(())
// }

// fn login(easy: &mut Easy, ip: &str) -> Result<(), MinerError> {
//     let url = LOGIN_URL.replace("{}", ip);
//     info!("avalon login url: {}", url);

//     let mut params = HashMap::new();
//     params.insert("username", "root");
//     params.insert("passwd", "root");

//     easy.url(&url)?;
//     easy.post(true)?;

//     let mut list = List::new();
//     list.append("Content-Type: application/x-www-form-urlencoded")?;
//     easy.http_headers(list)?;

//     let mut data = String::new();
//     for (key, value) in params {
//         data.push_str(&format!("{}={}&", key, value));
//     }
//     data.pop(); // remove the last '&'
//     easy.post_fields_copy(data.as_bytes())?;

//     let mut response_body = Vec::new();
//     {
//         let mut transfer = easy.transfer();
//         transfer.write_function(|new_data| {
//             response_body.extend_from_slice(new_data);
//             Ok(new_data.len())
//         })?;
//         transfer.perform()?;
//     }

//     easy.perform()?;

//     info!("avalon login status: {:?}", easy.response_code()?);
//     // log out body
//     let _body = String::from_utf8(response_body)?;
//     //info!("Response body: {}", body);

//     Ok(())
// }

fn query_miner_type(ip: &str) -> Result<serde_json::Value, MinerError> {
    let url = MINER_TYPE_URL.replace("{}", ip);
    //info!("avalon query_minet_type url: {}", url);

    let mut easy = Easy::new();
    easy.url(&url)?;
    easy.post(false)?;
    let mut data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            data.extend_from_slice(new_data);
            Ok(new_data.len())
        })?;
        transfer.perform()?;
    }
    easy.perform()?;
    let body = String::from_utf8(data).unwrap();

    let re = Regex::new(r"minerinfoCallback\((\{.*\})\)").unwrap();
    match re.captures(&body) {
        Some(caps) => {
            let target = caps.get(1).unwrap().as_str();
            // convert to json
            let json: serde_json::Value = serde_json::from_str(target)?;
            //info!("avalon query_minet_type result: {:?}", json);
            Ok(json)
        }
        None => Err(MinerError::ReadAvalonConfigError),
    }
}

fn query_machine(ip: &str) -> Result<serde_json::Value, MinerError> {
    let url = STATUS_URL.replace("{}", ip);
    //info!("avalon query_machine url: {}", url);

    let mut easy = Easy::new();
    easy.url(&url)?;
    easy.post(false)?;
    let mut data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            data.extend_from_slice(new_data);
            Ok(new_data.len())
        })?;
        transfer.perform()?;
    }
    easy.perform()?;
    let body = String::from_utf8(data).unwrap();

    let re = Regex::new(r"homeCallback\((\{.*?\})\)").unwrap();
    match re.captures(&body) {
        Some(caps) => {
            let target = caps.get(1).unwrap().as_str();
            //info!("target: {}", target);
            // convert to json
            let json: serde_json::Value = serde_json::from_str(target)?;
            //info!("avalon query_machine result: {:?}", json);
            Ok(json)
        }
        None => Err(MinerError::ReadAvalonConfigError),
    }
}

fn get_config(easy: &mut Easy, ip: &str) -> Result<Option<AvalonConfig>, MinerError> {
    let url = INFO_URL.replace("{}", ip) + rand::random::<u64>().to_string().as_str();
    //info!("avalon get_config url: {}", url);

    easy.url(&url)?;
    easy.post(false)?;
    let mut data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            data.extend_from_slice(new_data);
            Ok(new_data.len())
        })?;
        transfer.perform()?;
    }
    easy.perform()?;
    let body = String::from_utf8(data).unwrap();

    let re = Regex::new(r"CGConfCallback\((\{.*\})\)").unwrap();
    match re.captures(&body) {
        Some(caps) => {
            let target = caps.get(1).unwrap().as_str();
            // convert to json
            let config: AvalonConfig = serde_json::from_str(target)?;
            //info!("avalon get_config result: {:?}", config);
            Ok(Some(config))
        }
        None => Err(MinerError::ReadAvalonConfigError),
    }
}

fn update_miner_config(easy: &mut Easy, ip: &str, conf: &AvalonConfig) -> Result<(), MinerError> {
    // post conf as www-form-urlencoded
    let url = CONFIG_UPDATE_URL.replace("{}", ip); // + rand::random::<u64>().to_string().as_str();
                                                   //info!("avalon update_miner_config url: {}", url);

    easy.url(&url)?;
    easy.post(true)?;

    let mut list = List::new();
    list.append("Content-Type: application/x-www-form-urlencoded")?;
    easy.http_headers(list)?;

    let data = conf.to_form_string()?;
    easy.post_fields_copy(data.as_bytes())?;

    let mut response_body = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            response_body.extend_from_slice(new_data);
            Ok(new_data.len())
        })?;
        transfer.perform()?;
    }

    easy.perform()?;

    //info!("avalon update config result: {:?}", easy.response_code()?);
    // log out body
    let _body = String::from_utf8(response_body)?;

    Ok(())
    // regular extract target  from ...CGConfCallback({target}}...
}

fn reboot(easy: &mut Easy, ip: &str) -> Result<(), MinerError> {
    // use tcp interface to reboot
    tcp_write_reboot(ip)?;
    return Ok(());

    let url = REBOOT_URL.replace("{}", ip);
    //info!("avalon reboot url: {}", url);

    easy.url(&url)?;
    easy.post(true)?;

    let mut list = List::new();
    list.append("Content-Type: application/x-www-form-urlencoded")?;
    easy.http_headers(list)?;

    let data = "reboot=1";
    easy.post_fields_copy(data.as_bytes())?;
    // set timeout to 5s
    easy.timeout(Duration::from_secs(5))?;

    match easy.perform() {
        Ok(_) => (),
        Err(e) => {
            //info!("avalon reboot error: {:?}", e);
            if e.code() == 28 || e.code() == 56 {
                // timeout or connection reset is ok
            } else {
                return Err(MinerError::CurlError(e));
            }
        }
    }

    //info!("avalon reboot status: {:?}", easy.response_code()?);

    Ok(())
}

fn tcp_cmd(ip: &str, port: u16, cmd: &str, is_waiting_write: bool) -> Result<String, MinerError> {
    let addr = format!("{}:{}", ip, port);
    let addrs = addr.to_socket_addrs()?.next().unwrap();

    let mut stream = std::net::TcpStream::connect_timeout(&addrs, Duration::from_secs(3))?;
    stream.write_all(cmd.as_bytes())?;
    //info!("write done for cmd {}", cmd);

    if is_waiting_write {
        let mut buf = [0; 10240];
        let n = stream.read(&mut buf)?;
        let res = String::from_utf8(buf[..n].to_vec())?;
        //info!("avalon tcp_query result: {}", res);
        return Ok(res);
    }
    return Ok("".to_string());
}

/// query version
fn tcp_query_version(ip: &str) -> Result<String, MinerError> {
    tcp_cmd(ip, 4028, "version", true)
}

/// query pool
fn tcp_query_account(ip: &str) -> Result<String, MinerError> {
    let pool = tcp_cmd(ip, 4028, "pools", true)?;
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

/// reboot machine
fn tcp_write_reboot(ip: &str) -> Result<String, MinerError> {
    tcp_cmd(ip, 4028, "ascset|0,reboot,0", false) // cgminer-api-restart
}

/// config pool
// fn tcp_write_pool(ip: &str, cfg: &AvalonConfig) -> Result<String, MinerError> {
//     tcp_cmd(ip, 4028, "ascset|0,config,0", true) // cgminer-api-config
// }

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
        let mut easy = Easy::new();
        let config = get_config(&mut easy, ip).unwrap().unwrap();
        assert_eq!(config.mode, 1);
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
            let result = miner
                .switch_account_if_diff(ip.to_string(), account, true)
                .await;
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
        let mut easy = Easy::new();
        let res = reboot(&mut easy, ip).unwrap();
        assert_eq!(res, ());
    }

    #[tokio::test]
    async fn avalon_test_query() {
        let _ = *SETUP;
        let ip = "192.168.189.207";
        let miner = AvalonMiner {};
        let info = miner.query(ip.to_string()).unwrap();
        info!("avalon info: {:?}", info);
    }

    #[test]
    fn avalon_tcp_query_version() {
        let _ = *SETUP;
        let ip = "192.168.187.186";
        let res = tcp_query_version(ip).unwrap();
        assert!(res.contains("STATUS"));
    }

    #[test]
    fn avalon_tcp_cmd_reboot() {
        let _ = *SETUP;
        let ip = "192.168.189.213";
        let _res = tcp_write_reboot(ip).unwrap();
        assert!(true);
    }

    #[test]
    fn avalon_tcp_query_account() {
        let _ = *SETUP;
        let ip = "192.168.187.182";
        let res = tcp_query_account(ip).unwrap();
        assert!(true);
    }
}
