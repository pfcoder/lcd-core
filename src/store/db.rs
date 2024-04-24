use std::path::Path;

use crate::miner::entry::{Account, Machine, MinerStatus};
use log::info;
use rusqlite::{params, Connection};
use std::fs;

use crate::error::MinerError;

/// Sqlite DB
pub struct DB {
    conn: Connection,
    db_path: String,
}

impl DB {
    pub fn new(app_path: &str) -> Result<Self, MinerError> {
        info!("init sqlite db {}", app_path);

        let db_path = get_db_path(app_path);

        if !db_file_exists(&db_path) {
            create_db_file(app_path);
        }

        let conn = Connection::open(&db_path).unwrap();

        // main table of miners
        conn.execute(
            "CREATE TABLE IF NOT EXISTS t_machine (
                  id              INTEGER PRIMARY KEY,
                  account_id      INTEGER NOT NULL,
                  ip              TEXT NOT NULL UNIQUE,
                  name            TEXT NOT NULL,
                  model           TEXT,
                  status          TEXT NOT NULL,
                  run_mode        TEXT,
                  additional_info TEXT,
                  )",
            [],
        )?;

        // account table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS t_account (
                  id              INTEGER PRIMARY KEY,
                  name            TEXT NOT NULL,
                  password        TEXT NOT NULL,
                  pool1           TEXT NOT NULL,
                  pool2           TEXT NOT NULL,
                  pool3           TEXT,
                  run_mode        TEXT NOT NULL,
                  )",
            [],
        )?;

        Ok(Self { conn, db_path })
    }

    pub fn query_machine(&self, ip: &str) -> Result<Machine, MinerError> {
        // query db by ip
        let mut stmt = self.conn.prepare(
            "
                SELECT t_machine.*, t_account.* 
                FROM t_miner 
                JOIN t_account ON t_miner.account_id = t_account.id 
                WHERE t_miner.ip = ?1
            ",
        )?;
        let mut machine_iter = stmt.query_map(params![ip], |row| {
            Ok(Machine {
                id: row.get(0)?,
                account_id: row.get(1)?,
                ip: row.get(2)?,
                name: row.get(3)?,
                status: MinerStatus::Offline, //row.get(4)?.into(),
                run_mode: row.get(5)?,
                addition_info: row.get(6)?,
                account: Account {
                    id: row.get(7)?,
                    name: row.get(8)?,
                    password: row.get(9)?,
                    pool1: row.get(10)?,
                    pool2: row.get(11)?,
                    pool3: row.get(12)?,
                    run_mode: row.get(13)?,
                },
                switch_account: None,
            })
        })?;

        // get first
        match machine_iter.next() {
            Some(Ok(machine)) => Ok(machine),
            other => {
                info!("query machine not found: {:?}", other);
                Err(MinerError::MinerNotSupportError)
            }
        }
    }

    pub fn insert_miner(&self, machine: &Machine) -> Result<i32, MinerError> {
        // insert miner
        self.conn.execute(
            "INSERT INTO t_miner (account_id, ip, name, status) VALUES (?1, ?2, ?3, ?4)",
            params![machine.account_id, machine.ip, machine.name, "0"],
        )?;

        // return miner id
        Ok(self.conn.last_insert_rowid() as i32)
    }

    pub fn insert_account(&self, account: &Account) -> Result<i32, MinerError> {
        // insert account
        self.conn.execute(
            "INSERT INTO t_account (name, password, pool1, pool2, pool3) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![account.name, account.password, account.pool1, account.pool2, account.pool3],
        )?;

        // return account id
        Ok(self.conn.last_insert_rowid() as i32)
    }
}

fn create_db_file(app_path: &str) {
    let db_path = get_db_path(app_path);
    let db_dir = Path::new(&db_path).parent().unwrap();

    if !db_dir.exists() {
        fs::create_dir_all(db_dir).unwrap();
    }

    fs::File::create(db_path).unwrap();
}

fn db_file_exists(db_path: &str) -> bool {
    Path::new(&db_path).exists()
}

fn get_db_path(app_path: &str) -> String {
    app_path.to_owned() + "/db/lcd.sqlite"
}
