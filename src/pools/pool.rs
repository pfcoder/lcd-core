use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{error::MinerError, store::db};

use super::{f2pool::F2pool, poolin::Poolin};

pub enum PoolType {
    Poolin(Poolin),
    F2pool(F2pool),
}

impl PoolType {
    pub fn detect(watcher_url: &str) -> Result<PoolType, MinerError> {
        if watcher_url.contains("poolin") {
            return Ok(PoolType::Poolin(Poolin::from_watcher(watcher_url)?));
        }
        if watcher_url.contains("f2pool") {
            return Ok(PoolType::F2pool(F2pool::from_watcher(watcher_url)?));
        }
        Err(MinerError::PoolTypeNotDetected)
    }
}

/// public data define
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PoolWorker {
    pub name: String,
    pub hash_real: f64,
    pub hash_avg: f64,
    pub time_stamp: i64,
    pub pool_type: String,
}

// define trait for general pool api query
pub trait Pool {
    async fn query(&self) -> Result<Vec<PoolWorker>, MinerError>;
}

impl Pool for PoolType {
    async fn query(&self) -> Result<Vec<PoolWorker>, MinerError> {
        match self {
            PoolType::Poolin(poolin) => poolin.query().await,
            PoolType::F2pool(f2pool) => f2pool.query().await,
        }
    }
}

pub async fn query_pool_workers(
    watcher_url: &str,
    f2p_account: &str,
    f2p_secret: &str,
) -> Result<Vec<PoolWorker>, MinerError> {
    let mut workers = vec![];
    // detect pool type
    if watcher_url.contains("poolin") {
        match PoolType::detect(watcher_url) {
            Ok(pool) => {
                // get query result, ignore error, return empty vec
                let w = match pool.query().await {
                    Ok(result) => result,
                    Err(_) => vec![],
                };
                workers.extend(w);
            }
            Err(e) => {
                error!("detect pool type error: {:?}", e);
            }
        }
    }

    if f2p_account.len() > 0 && f2p_secret.len() > 0 {
        let f2pool = F2pool::from_account(f2p_account.to_string(), f2p_secret.to_string());
        let w = match f2pool.query().await {
            Ok(result) => result,
            Err(_) => vec![],
        };
        workers.extend(w);
    }

    Ok(workers)
}

pub fn schedule_query_task(
    runtime: tokio::runtime::Handle,
    watcher_url: String,
    f2p_account: String,
    f2p_secret: String,
) -> tokio::task::JoinHandle<()> {
    // create tokio runtime context
    return runtime.spawn(async move {
        loop {
            info!("query pool workers task scheduled.");
            let workers = query_pool_workers(&watcher_url, &f2p_account, &f2p_secret).await;
            match workers {
                Ok(workers) => {
                    // update db
                    for worker in workers {
                        match db::insert_pool_record(
                            &worker.name,
                            worker.hash_real,
                            worker.hash_avg,
                            &worker.pool_type,
                            worker.time_stamp,
                        ) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("insert pool record error: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("query pool workers error: {:?}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
        }
    });
}
