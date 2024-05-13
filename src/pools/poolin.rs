use log::info;
use regex::Regex;
/// poolin.one api query
use serde::{Deserialize, Serialize};

use super::pool::{Pool, PoolWorker};
use crate::error::MinerError;
use reqwest::header;

pub struct Poolin {
    pub api_url: String,
    pub token: String,
}

impl Poolin {
    pub fn from_watcher(watcher: &str) -> Result<Self, MinerError> {
        // https://www.poolin.one/my/9382015/btc/dashboard?read_token=wowpYnza1WuonEvbTlu3Phamh2FlxWBcrxZPFjbOm0nOkKUt6Jbs7OyGmKEyUMPd
        // extract 9382015 and wowpYnza1WuonEvbTlu3Phamh2FlxWBcrxZPFjbOm0nOkKUt6Jbs7OyGmKEyUMPd

        let re = Regex::new(r"\/my\/(\d+)\/.*read_token=(.*)").unwrap();
        let caps = re.captures(watcher).unwrap();
        if caps.len() != 3 {
            return Err(MinerError::PoolinApiRegexError);
        }
        let uid = caps.get(1).unwrap().as_str();
        let token = caps.get(2).unwrap().as_str();

        // https://api-prod.poolin.one/api/public/v2/worker?status=ALL&puid=9382015&coin_type=btc&sort=asc&order_by=worker_name&pagesize=100
        Ok(Poolin {
            api_url: format!("https://api-prod.poolin.one/api/public/v2/worker?status=ALL&puid={}&coin_type=btc&sort=asc&order_by=worker_name", uid),
            token: token.to_string(),
        })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PoolinWorker {
    pub worker_name: String,
    pub shares_15m: f64,
    pub shares_24h: f64,
    pub last_share_time: i64,
    pub shares_unit: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PoolinPageData {
    pub page: i32,
    pub page_size: i32,
    pub page_count: i32,
    pub total_count: i32,
    pub data: Vec<PoolinWorker>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PoolinResponse {
    pub err_no: i32,
    pub data: PoolinPageData,
}

// impl convert from PoolinWorker to PoolWorker
impl From<PoolinWorker> for PoolWorker {
    fn from(pw: PoolinWorker) -> Self {
        PoolWorker {
            name: pw.worker_name,
            hash_real: pw.shares_15m,
            hash_avg: pw.shares_24h,
            time_stamp: pw.last_share_time,
            pool_type: "poolin".to_string(),
        }
    }
}

impl Pool for Poolin {
    async fn query(&self) -> Result<Vec<PoolWorker>, MinerError> {
        let mut workers = vec![];

        // page query all data from poolin
        let page_size = 100;
        let mut page = 1;
        loop {
            let resp = self
                .query_poolin_api(&self.api_url, page, page_size)
                .await?;
            if resp.err_no != 0 {
                return Err(MinerError::PoolinApiRequestError);
            }

            for worker in resp.data.data {
                workers.push(worker.into());
            }

            info!(
                "page: {}, page_size: {}, page_count: {}, total_count: {}",
                resp.data.page, resp.data.page_size, resp.data.page_count, resp.data.total_count
            );

            if resp.data.page >= resp.data.page_count {
                break;
            }

            page += 1;
        }

        Ok(workers)
    }
}

impl Poolin {
    pub async fn query_poolin_api(
        &self,
        url: &str,
        page: i32,
        page_size: i32,
    ) -> Result<PoolinResponse, MinerError> {
        let client = reqwest::Client::new();

        let resp: PoolinResponse = client
            .get(format!("{}&page={}&pagesize={}", url, page, page_size))
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .send()
            .await?
            .json::<PoolinResponse>()
            .await?;

        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use log::info;

    use super::*;

    lazy_static! {
        static ref SETUP: () = {
            env_logger::init();
        };
    }

    #[test]
    fn test_poolin_from_watcher() {
        let _ = &*SETUP;
        let watcher = "https://www.poolin.one/my/9382015/btc/dashboard?read_token=wowpYnza1WuonEvbTlu3Phamh2FlxWBcrxZPFjbOm0nOkKUt6Jbs7OyGmKEyUMPd";
        let poolin = Poolin::from_watcher(watcher).unwrap();
        assert_eq!(poolin.api_url, "https://api-prod.poolin.one/api/public/v2/worker?status=ALL&puid=9382015&coin_type=btc&sort=asc&order_by=worker_name");
        assert_eq!(
            poolin.token,
            "wowpYnza1WuonEvbTlu3Phamh2FlxWBcrxZPFjbOm0nOkKUt6Jbs7OyGmKEyUMPd"
        );
    }

    #[tokio::test]
    async fn test_poolin_query() {
        let _ = &*SETUP;
        let watcher = "https://www.poolin.one/my/9273101/btc/dashboard?read_token=wowUx0bw6YzPQfdijDDdduSeI2ueMUsKRWgCLcbl6hUWXq3lr9JVcpqHEq2KAqmh";
        let poolin = Poolin::from_watcher(watcher).unwrap();
        let workers = poolin.query().await.unwrap();
        info!("workers: {:?}", workers);
        assert_eq!(workers.len(), 27);
    }
}
