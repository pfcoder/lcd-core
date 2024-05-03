use std::{path::Path, sync::Mutex};

use crate::miner::entry::MachineRecord;
use log::info;
use rusqlite::{params, Connection};
use std::fs;

use crate::error::MinerError;

lazy_static! {
    static ref LCD_DB: Mutex<Option<DB>> = Mutex::new(None);
}

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
            "CREATE TABLE IF NOT EXISTS t_machine_record (
                  id              INTEGER PRIMARY KEY,
                  ip              TEXT NOT NULL,
                  machine_type    TEXT,
                  work_mode       INTEGER,
                  hash_real       REAL,
                  hash_avg        REAL,
                  temp_0          REAL,
                  temp_1          REAL,
                  temp_2          REAL,
                  power           INTEGER,
                  create_time     INTEGER
                  )",
            [],
        )?;

        Ok(Self { conn, db_path })
    }

    pub fn insert_machine_record(&self, machine: &MachineRecord) -> Result<i32, MinerError> {
        // insert miner
        self.conn.execute(
            "INSERT INTO t_machine_record (ip, machine_type, work_mode, hash_real, hash_avg, temp_0, temp_1, temp_2, power, create_time)
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                machine.ip,
                machine.machine_type,
                machine.work_mode,
                machine.hash_real,
                machine.hash_avg,
                machine.temp_0,
                machine.temp_1,
                machine.temp_2,
                machine.power,
                machine.create_time
            ],
        )?;

        // return miner id
        Ok(self.conn.last_insert_rowid() as i32)
    }

    pub fn query_machine_records_by_time(
        &self,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<MachineRecord>, MinerError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ip, machine_type, work_mode, hash_real, hash_avg, temp_0, temp_1, temp_2, power, create_time
                  FROM t_machine_record
                  WHERE create_time >= ?1 AND create_time <= ?2",
        )?;

        let rows = stmt.query_map(params![start_time, end_time], |row| {
            Ok(MachineRecord {
                id: row.get(0)?,
                ip: row.get(1)?,
                machine_type: row.get(2)?,
                work_mode: row.get(3)?,
                hash_real: row.get(4)?,
                hash_avg: row.get(5)?,
                temp_0: row.get(6)?,
                temp_1: row.get(7)?,
                temp_2: row.get(8)?,
                power: row.get(9)?,
                create_time: row.get(10)?,
            })
        })?;

        let mut machines = Vec::new();
        for machine in rows {
            machines.push(machine?);
        }

        Ok(machines)
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

pub fn init(app_path: &str) {
    let mut db = LCD_DB.lock().unwrap();
    let db_inst = DB::new(app_path).unwrap();
    *db = Some(db_inst);

    info!("lcd db initialized.");
}

pub fn insert_machine_record(machine: &MachineRecord) -> Result<i32, MinerError> {
    let db = LCD_DB.lock().unwrap();
    match &*db {
        Some(db) => db.insert_machine_record(machine),
        None => Ok(-1),
    }
}
