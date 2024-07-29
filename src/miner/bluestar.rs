use crate::error::MinerError;

use super::entry::*;
//use log::info;

/// Avalon miner
#[derive(Debug, Clone)]
pub struct BlueStarMiner {}

impl MinerOperation for BlueStarMiner {
    fn info(&self) -> MinerInfo {
        MinerInfo {
            name: "bluestar".to_string(),
            detail: "BlueStar is a miner".to_string(),
        }
    }

    fn detect(&self, _headers: Vec<String>, _body: &str) -> Result<MinerType, MinerError> {
        Err(MinerError::MinerNotSupportError)
    }

    fn switch_account_if_diff(
        &self,
        _ip: &str,
        _account: &Account,
        _is_force: bool,
    ) -> AsyncOpType<()> {
        todo!()
    }

    fn query(&self, _ip: &str, _timeout_seconds: i64) -> Result<MachineInfo, MinerError> {
        todo!()
    }

    fn reboot(&self, _ip: &str) -> Result<(), MinerError> {
        todo!()
    }

    fn config_pool(&self, _ip: &str, _pools: &Vec<PoolConfig>) -> Result<(), MinerError> {
        todo!()
    }

    fn config_mode(&self, ip: &str, mode: &str) -> Result<(), MinerError> {
        todo!()
    }

    fn config(&self, ip: &str, mode: &str, pools: &Vec<PoolConfig>) -> Result<(), MinerError> {
        todo!()
    }
}
