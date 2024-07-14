use log::{debug, error};
use rusqlite::Connection;

use std::convert::TryFrom;

#[derive(Debug)]
pub struct StatisticApi {
    conn: Connection,
}
impl StatisticApi {
    /// Create StatisticApi instance
    pub fn new(conn: Connection) -> Self {
        StatisticApi { conn }
    }

    pub fn save(&self, id: u32) -> anyhow::Result<()> {
        let sql = "INSERT INTO downloads VALUES($1, datetime('now', 'localtime'));";
        let mut statement = self.conn.prepare_cached(sql)?;
        let _ = statement.execute([id])?;
        Ok(())
    }

    pub fn load_last(&self, days: u32) -> anyhow::Result<Vec<u32>> {
        let days = format!("-{days} days");

        let sql = "SELECT book_id AS id FROM downloads WHERE DATE(downloaded) >= DATE('now', $1);";
        let mut statement = self.conn.prepare_cached(sql)?;
        let idx = statement.column_index("id")?;
        let rows = statement.query_map([days], |row| row.get(idx))?;

        let mut ids = Vec::new();
        for id in rows {
            ids.push(id?);
        }
        Ok(ids)
    }

    /// Returns true if database opened in ReadOnly
    pub fn is_readonly(&self) -> anyhow::Result<bool> {
        Ok(self.conn.is_readonly(rusqlite::DatabaseName::Main)?)
    }
}
impl TryFrom<&str> for StatisticApi {
    type Error = anyhow::Error;

    fn try_from(database: &str) -> anyhow::Result<Self> {
        debug!("database: {database}");
        let conn = Connection::open(database).inspect_err(|e| error!("{e}"))?;
        conn.execute_batch(
            r#"
        CREATE TABLE IF NOT EXISTS downloads(
            book_id     INTEGER NOT NULL,
            downloaded  DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(book_id) ON CONFLICT REPLACE);
        "#,
        )?;

        Ok(Self::new(conn))
    }
}
impl TryFrom<&String> for StatisticApi {
    type Error = anyhow::Error;

    fn try_from(database: &String) -> anyhow::Result<Self> {
        debug!("database: {database}");
        StatisticApi::try_from(database.as_str())
    }
}

