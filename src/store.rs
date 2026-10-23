use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

#[derive(Clone, Debug)]
pub struct Phrase {
    pub id: i64,
    pub phrase: String,
}

#[derive(Clone, Debug)]
pub struct HistoryItem {
    pub id: i64,
    pub transcript: String,
    pub polished: String,
    pub bundle_id: String,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open() -> Result<Self> {
        let dir = dirs::data_dir()
            .context("找不到本机数据目录")?
            .join("OpenTypeless");
        std::fs::create_dir_all(dir.join("audio"))?;
        let conn = Connection::open(dir.join("opentypeless.db"))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS phrases (
                id INTEGER PRIMARY KEY,
                phrase TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY,
                created_at INTEGER NOT NULL,
                bundle_id TEXT NOT NULL,
                transcript TEXT NOT NULL,
                polished TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(Self { conn })
    }

    pub fn show_dock(&self) -> bool {
        self.setting("show_dock").as_deref() != Some("0")
    }

    pub fn set_show_dock(&self, show: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES ('show_dock', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![if show { "1" } else { "0" }],
        )?;
        Ok(())
    }

    fn setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn audio_path(&self, id: i64) -> std::path::PathBuf {
        dirs::data_dir()
            .unwrap_or_default()
            .join("OpenTypeless")
            .join("audio")
            .join(format!("{id}.wav"))
    }

    pub fn phrases(&self) -> Result<Vec<Phrase>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, phrase FROM phrases ORDER BY id DESC LIMIT 100")?;
        let rows = stmt.query_map([], |row| {
            Ok(Phrase {
                id: row.get(0)?,
                phrase: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn phrase_list(&self) -> Vec<String> {
        self.phrases()
            .unwrap_or_default()
            .into_iter()
            .rev()
            .map(|item| item.phrase)
            .take(100)
            .collect()
    }

    pub fn add_phrase(&self, phrase: &str) -> Result<()> {
        let phrase = phrase.trim();
        if phrase.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO phrases (phrase, created_at) VALUES (?1, ?2)",
            params![phrase, now()],
        )?;
        Ok(())
    }

    pub fn delete_phrase(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM phrases WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_history(&self, bundle_id: &str, transcript: &str, polished: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history (created_at, bundle_id, transcript, polished) VALUES (?1, ?2, ?3, ?4)",
            params![now(), bundle_id, transcript, polished],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn history(&self) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, transcript, polished, bundle_id FROM history ORDER BY id DESC LIMIT 20",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                transcript: row.get(1)?,
                polished: row.get(2)?,
                bundle_id: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
