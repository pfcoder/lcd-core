use crate::error::MinerError;

use super::pool::{Pool, PoolWorker};

pub struct F2pool {
    api_url: String,
    token: String,
}

impl Pool for F2pool {
    async fn query(&self) -> Result<Vec<PoolWorker>, MinerError> {
        Ok(vec![])
    }
}

impl F2pool {
    pub fn from_watcher(watcher_url: &str) -> Result<F2pool, MinerError> {
        Err(MinerError::PoolTypeNotDetected)
    }
}
