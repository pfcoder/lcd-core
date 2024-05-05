use std::time::Duration;

use super::entry::*;
use crate::error::MinerError;
use curl::easy::{Easy, List};
//use log::info;
use serde::{Deserialize, Serialize};

// some const str define
const CONF_URL: &str = "http://{}/cgi-bin/get_miner_conf.cgi";
const UPDATE_URL: &str = "http://{}/cgi-bin/set_miner_conf.cgi";

// AntConfig
// {
//     "pools" : [
//     {
//     "url" : "192.168.190.9:9011",
//     "user" : "sl002.189x183",
//     "pass" : "123"
//     },
//     {
//     "url" : "192.168.190.8:9011",
//     "user" : "sl002.189x183",
//     "pass" : "123"
//     },
//     {
//     "url" : "192.168.190.8:9011",
//     "user" : "sl002.189x183",
//     "pass" : ""
//     }
//     ]
//     ,
//     "api-listen" : true,
//     "api-network" : true,
//     "api-groups" : "A:stats:pools:devs:summary:version",
//     "api-allow" : "A:0/0,W:*",
//     "bitmain-fan-ctrl" : false,
//     "bitmain-fan-pwm" : "100",
//     "bitmain-use-vil" : true,
//     "bitmain-freq" : "675",
//     "bitmain-voltage" : "1400",
//     "bitmain-ccdelay" : "0",
//     "bitmain-pwth" : "0",
//     "bitmain-work-mode" : "0",
//     "bitmain-freq-level" : "100"
//     }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pool {
    url: String,
    user: String,
    pass: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AntConfig {
    pools: Vec<Pool>,
    #[serde(rename = "api-listen")]
    api_listen: bool,
    #[serde(rename = "api-network")]
    api_network: bool,
    #[serde(rename = "api-groups")]
    api_groups: String,
    #[serde(rename = "api-allow")]
    api_allow: String,
    #[serde(rename = "bitmain-fan-ctrl")]
    bitmain_fan_ctrl: bool,
    #[serde(rename = "bitmain-fan-pwm")]
    bitmain_fan_pwm: String,
    #[serde(rename = "bitmain-use-vil")]
    bitmain_use_vil: bool,
    #[serde(rename = "bitmain-freq")]
    bitmain_freq: String,
    #[serde(rename = "bitmain-voltage")]
    bitmain_voltage: String,
    #[serde(rename = "bitmain-ccdelay")]
    bitmain_ccdelay: String,
    #[serde(rename = "bitmain-pwth")]
    bitmain_pwth: String,
    #[serde(rename = "bitmain-work-mode")]
    bitmain_work_mode: String,
    #[serde(rename = "bitmain-freq-level")]
    bitmain_freq_level: String,
}

impl AntConfig {
    pub fn is_same_account(&self, account: &Account) -> bool {
        // check string before .
        let worker1_splited: Vec<&str> = self.pools[0].user.split('.').collect();
        let account_name_splited: Vec<&str> = account.name.split('.').collect();
        worker1_splited[0] == account_name_splited[0]
    }

    pub fn apply_account(&mut self, account: &Account, ip: &str) {
        let ip_splited: Vec<&str> = ip.split('.').collect();
        let user = account.name.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
        self.pools[0].user = user.clone();
        self.pools[0].pass = account.password.clone();
        self.pools[0].url = account.pool1.clone();
        self.pools[1].user = user.clone();
        self.pools[1].pass = account.password.clone();
        self.pools[1].url = account.pool2.clone();
        self.pools[2].user = user.clone();
        self.pools[2].pass = account.password.clone();
        self.pools[2].url = account.pool2.clone();
    }

    pub fn apply_config_pools(&mut self, pools: Vec<PoolConfig>, ip: &str) {
        let ip_splited: Vec<&str> = ip.split('.').collect();
        for (i, pool) in pools.iter().enumerate() {
            let user = pool.user.clone() + "." + ip_splited[2] + "x" + ip_splited[3];
            self.pools[i].user = user.clone();
            self.pools[i].pass = pool.password.clone();
            self.pools[i].url = pool.url.clone();
        }
    }
}

/// Ant miner
#[derive(Debug, Clone)]
pub struct AntMiner {}

impl MinerOperation for AntMiner {
    fn info(&self) -> MinerInfo {
        MinerInfo {
            name: "ant".to_string(),
            detail: "Antminer is a miner".to_string(),
        }
    }

    fn detect(&self, headers: Vec<String>, _body: &str) -> Result<MinerType, MinerError> {
        if headers.len() < 1 {
            return Err(MinerError::MinerNotSupportError);
        }

        for header in headers {
            if header.contains("antMiner") {
                return Ok(MinerType::Ant(AntMiner {}));
            }
        }

        Err(MinerError::MinerNotSupportError)
    }

    fn switch_account_if_diff(
        &self,
        ip: String,
        account: Account,
        is_force: bool,
    ) -> AsyncOpType<()> {
        Box::pin(async move {
            let mut conf = get_conf(&ip)?;

            if !is_force && conf.is_same_account(&account) {
                // info!(
                //     "ant account not changed: {} current:{} switch:{}",
                //     ip, conf.pools[0].user, account.name
                // );
                return Ok(());
            }
            conf.apply_account(&account, &ip);
            update_conf(&ip, &conf)?;
            reboot(&ip)?;

            Ok(())
        })
    }

    fn query(&self, ip: String, _timeout_seconds: i64) -> Result<MachineInfo, MinerError> {
        let json = query_machine(&ip)?;
        let conf = get_conf(&ip)?;

        let machine_type = json["INFO"]["type"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let elapsed = json["STATS"][0]["elapsed"].as_i64().unwrap_or(0);
        let hash_real = json["STATS"][0]["rate_5s"].as_f64().unwrap_or(0.0);
        let hash_avg = json["STATS"][0]["rate_avg"].as_f64().unwrap_or(0.0);
        // elapsed is seconds, convert to H:M:S
        let elapsed_str = format!(
            "{}H {}M {}S",
            elapsed / 3600,
            (elapsed % 3600) / 60,
            elapsed % 60
        );

        // construct MachineInfo
        Ok(MachineInfo {
            ip: ip.clone(),
            elapsed: elapsed_str,
            hash_real: format!("{:.3} GH/s", hash_real),
            hash_avg: format!("{:.3} GH/s", hash_avg),
            machine_type: machine_type.clone(),
            temp: "0".to_string(),
            fan: "0".to_string(),
            mode: "".to_string(),
            pool1: conf.pools[0].url.clone(),
            worker1: conf.pools[0].user.clone(),
            pool2: conf.pools[1].url.clone(),
            worker2: conf.pools[1].user.clone(),
            record: MachineRecord {
                id: 0,
                ip: ip,
                machine_type,
                work_mode: 0,
                hash_real,
                hash_avg,
                temp_0: 0.0,
                temp_1: 0.0,
                temp_2: 0.0,
                power: 0,
                create_time: chrono::Local::now().timestamp(),
            },
        })
    }

    fn reboot(&self, ip: String) -> Result<(), MinerError> {
        Ok(reboot(&ip)?)
    }

    fn config_pool(&self, ip: String, pools: Vec<PoolConfig>) -> Result<(), MinerError> {
        let mut conf = get_conf(&ip)?;
        conf.apply_config_pools(pools, &ip);
        update_conf(&ip, &conf)?;
        reboot(&ip)
    }
}

fn query_machine(ip: &str) -> Result<serde_json::Value, MinerError> {
    let url = "http://{}/cgi-bin/stats.cgi".replace("{}", ip);

    let mut easy = Easy::new();
    easy.url(&url)?;

    easy.username("root")?;
    easy.password("root")?;

    let mut auth = curl::easy::Auth::new();
    auth.digest(true);

    easy.http_auth(&auth)?;

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

    let body = String::from_utf8(response_body)?;
    // convert to general json
    let json: serde_json::Value = serde_json::from_str(&body)?;

    //info!("ant info: {:?}", json);
    Ok(json)
}

fn get_conf(ip: &str) -> Result<AntConfig, MinerError> {
    let url = CONF_URL.replace("{}", ip);
    let mut easy = Easy::new();
    easy.url(&url)?;

    easy.username("root")?;
    easy.password("root")?;

    let mut auth = curl::easy::Auth::new();
    auth.digest(true);

    easy.http_auth(&auth)?;
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
    let body = String::from_utf8(response_body)?;

    let conf = serde_json::from_str::<AntConfig>(&body)?;

    //info!("ant conf: {:?}", conf);
    Ok(conf)
}

fn update_conf(ip: &str, conf: &AntConfig) -> Result<(), MinerError> {
    let url = UPDATE_URL.replace("{}", ip);
    let conf_str = serde_json::to_string(&conf)?;

    //info!("ant update conf: {}", conf_str);

    let mut easy = Easy::new();
    easy.url(&url)?;

    easy.username("root")?;
    easy.password("root")?;

    let mut auth = curl::easy::Auth::new();
    auth.digest(true);

    easy.http_auth(&auth)?;

    let mut list = List::new();

    list.append("Content-Type: text/plain;charset=UTF-8")?;
    easy.http_headers(list)?;

    easy.post(true)?;
    easy.post_fields_copy(conf_str.as_bytes())?;

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

    let _body = String::from_utf8(response_body)?;

    //info!("ant update conf: {}", body);
    Ok(())
}

fn reboot(ip: &str) -> Result<(), MinerError> {
    let url = "http://{}/cgi-bin/reboot.cgi".replace("{}", ip);

    let mut easy = Easy::new();
    easy.url(&url)?;

    easy.username("root")?;
    easy.password("root")?;

    let mut auth = curl::easy::Auth::new();
    auth.digest(true);

    easy.http_auth(&auth)?;

    easy.post(false)?;

    let mut response_body = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|new_data| {
            response_body.extend_from_slice(new_data);
            Ok(new_data.len())
        })?;
        transfer.perform()?;
    }

    easy.timeout(Duration::from_secs(5))?;

    match easy.perform() {
        Ok(_) => (),
        Err(_e) => {
            //info!("ant reboot error: {:?}", e);
        }
    }

    //info!("ant reboot: {}", body);
    Ok(())
}

// test
#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;
    use log::info;

    #[tokio::test]
    async fn ant_test_update_conf() {
        env_logger::try_init();
        let ip = "192.168.189.183";
        let mut conf = get_conf(ip).unwrap();
        conf.pools[0].pass = "1235".to_string();
        update_conf(ip, &conf).unwrap();
        assert!(true);
    }

    #[tokio::test]
    async fn ant_test_query() {
        env_logger::try_init();
        let ip = "192.168.190.231";
        let miner = AntMiner {};
        let info = miner.query(ip.to_string(), 3).unwrap();
        info!("ant info: {:?}", info);
        assert!(true);
    }
}
