use log::info;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use crate::error::MinerError;

use super::pool::{Pool, PoolWorker};

pub struct F2pool {
    api_url: String,
    account: String,
    secret: String,
}

/**
* "workers": [
       [
           "188x41",
           76936493634245.97,
           77561993582491.88,
           0,
           1190920626462785500,
           0,
           "2024-05-15T06:48:09.000Z",
           false,
           0,
           0,
           0
       ]
   ]
*/

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct F2poolWorker {
    pub name: String,
    pub hash_rate: f64,
    pub h1_hash_rate: f64,
    pub time_stamp: i64,
}

// impl converter from JSON array to F2poolWorker
impl From<serde_json::Value> for F2poolWorker {
    fn from(value: serde_json::Value) -> Self {
        let mut worker = F2poolWorker::default();
        if let Some(worker_array) = value.as_array() {
            if let (Some(name), Some(hash_rate), Some(h1_hash_rate), Some(_time_stamp)) = (
                worker_array.get(0).and_then(|v| v.as_str()),
                worker_array.get(1).and_then(|v| v.as_f64()),
                worker_array.get(2).and_then(|v| v.as_f64()),
                worker_array.get(6).and_then(|v| v.as_str()),
            ) {
                worker.name = name.to_string();
                // keep 3 decimal places
                worker.hash_rate = format!("{:.3}", hash_rate / 1000000000000.0)
                    .parse::<f64>()
                    .unwrap();
                worker.h1_hash_rate = format!("{:.3}", h1_hash_rate / 1000000000000.0)
                    .parse::<f64>()
                    .unwrap();
                // convert timestamp from 2024-05-15T06:48:09.000Z
                // worker.time_stamp = chrono::DateTime::parse_from_rfc3339(time_stamp)
                //     .unwrap()
                //     .timestamp();
                worker.time_stamp = chrono::Local::now().timestamp();
            }
        }

        worker
    }
}

impl From<F2poolWorker> for PoolWorker {
    fn from(fw: F2poolWorker) -> Self {
        PoolWorker {
            name: fw.name,
            hash_real: fw.hash_rate,
            hash_avg: fw.h1_hash_rate,
            time_stamp: fw.time_stamp,
            pool_type: "f2pool".to_string(),
        }
    }
}

impl Pool for F2pool {
    async fn query(&self, proxy: &str) -> Result<Vec<PoolWorker>, MinerError> {
        let client: Client;
        if !proxy.is_empty() {
            // if proxy not start with http, add it
            let proxy = if proxy.starts_with("http") {
                proxy.to_string()
            } else {
                format!("http://{}", proxy)
            };
            let proxy = reqwest::Proxy::all(proxy).unwrap();
            client = Client::builder().proxy(proxy).build()?;
        } else {
            client = Client::new();
        }

        info!("query f2pool workers: {}/{}", self.api_url, self.account);
        let resp = client
            .get(format!("{}/{}/{}", self.api_url, "bitcoin", self.account))
            .header(header::CONTENT_TYPE, "application/json")
            .header("F2P-API-SECRET", &self.secret)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let json_body = resp.json::<serde_json::Value>().await?;

        //info!("resp: {:?}", json_body.get("workers"));
        let workers: Vec<PoolWorker> = json_body
            .get("workers")
            .and_then(|v| v.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|v| PoolWorker::from(F2poolWorker::from(v.clone())))
            .collect();

        info!("workers: {:?}", workers);

        Ok(workers)
    }
}

impl F2pool {
    pub fn from_watcher(_watcher_url: &str) -> Result<F2pool, MinerError> {
        Err(MinerError::PoolTypeNotDetected)
    }

    pub fn from_account(account: String, secret: String) -> F2pool {
        F2pool {
            api_url: "https://api.f2pool.com".to_string(),
            account,
            secret,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use log::info;

    lazy_static! {
        static ref SETUP: () = {
            env_logger::init();
        };
    }

    #[tokio::test]
    async fn test_f2pool_query() {
        let _ = *SETUP;

        let f2pool = F2pool::from_account("x".to_string(), "x".to_string());

        let workers = f2pool.query("").await.unwrap();
        info!("workers: {:?}", workers);
        assert!(!workers.is_empty());
    }
}
