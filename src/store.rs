use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phrase {
    pub id: i64,
    pub phrase: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryItem {
    pub id: i64,
    pub created_at: i64,
    pub transcript: String,
    pub polished: String,
    pub bundle_id: String,
    pub duration_ms: i64,
    pub mode: String,
}

impl HistoryItem {
    pub fn display_text(&self) -> &str {
        if self.polished.trim().is_empty() {
            &self.transcript
        } else {
            &self.polished
        }
    }

    pub fn has_distinct_raw(&self) -> bool {
        let raw = self.transcript.trim();
        let pol = self.polished.trim();
        !raw.is_empty() && !pol.is_empty() && raw != pol
    }

    pub fn char_count(&self) -> usize {
        self.display_text().chars().count()
    }
}

#[derive(Clone, Debug, Default)]
pub struct StatsSummary {
    pub total_sessions: usize,
    pub total_chars: usize,
    pub total_duration_ms: i64,
    pub avg_cpm: usize,
    pub saved_minutes: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerMode {
    Toggle,
    Hold,
}

impl TriggerMode {
    pub fn from_str(raw: &str) -> Self {
        match raw {
            "hold" => Self::Hold,
            _ => Self::Toggle,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Hold => "hold",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Toggle => "单击切换（按一下开始，再按一下完成）",
            Self::Hold => "按住说话（按住录音，松开立即转写）",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Toggle => "单击切换",
            Self::Hold => "按住说话",
        }
    }
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open() -> Result<Self> {
        let dir = Self::data_dir()?;
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
                polished TEXT NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                mode TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        // Seamless schema migration for existing databases
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN mode TEXT NOT NULL DEFAULT ''",
            [],
        );
        Ok(Self { conn })
    }

    pub fn data_dir() -> Result<PathBuf> {
        Ok(dirs::data_dir()
            .context("找不到本机数据目录")?
            .join("OpenTypeless"))
    }

    pub fn show_dock(&self) -> bool {
        self.setting("show_dock").as_deref() != Some("0")
    }

    pub fn set_show_dock(&self, show: bool) -> Result<()> {
        self.set_setting("show_dock", if show { "1" } else { "0" })
    }

    pub fn hotkey_code(&self) -> i64 {
        self.setting("hotkey_code")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(61)
    }

    pub fn set_hotkey_code(&self, code: i64) -> Result<()> {
        self.set_setting("hotkey_code", &code.to_string())
    }

    pub fn trigger_mode(&self) -> TriggerMode {
        self.setting("trigger_mode")
            .map(|v| TriggerMode::from_str(&v))
            .unwrap_or(TriggerMode::Toggle)
    }

    pub fn set_trigger_mode(&self, mode: TriggerMode) -> Result<()> {
        self.set_setting("trigger_mode", mode.as_str())
    }

    pub fn polish_strategy(&self) -> String {
        self.setting("polish_strategy")
            .unwrap_or_else(|| "auto".to_string())
    }

    pub fn set_polish_strategy(&self, strategy: &str) -> Result<()> {
        self.set_setting("polish_strategy", strategy)
    }

    pub fn custom_prompt(&self) -> String {
        self.setting("custom_prompt").unwrap_or_default()
    }

    pub fn set_custom_prompt(&self, prompt: &str) -> Result<()> {
        self.set_setting("custom_prompt", prompt.trim())
    }

    pub fn space_cjk(&self) -> bool {
        self.setting("space_cjk").as_deref() != Some("0")
    }

    pub fn set_space_cjk(&self, enabled: bool) -> Result<()> {
        self.set_setting("space_cjk", if enabled { "1" } else { "0" })
    }

    pub fn sound_feedback(&self) -> bool {
        self.setting("sound_feedback").as_deref() != Some("0")
    }

    pub fn set_sound_feedback(&self, enabled: bool) -> Result<()> {
        self.set_setting("sound_feedback", if enabled { "1" } else { "0" })
    }

    pub fn save_audio(&self) -> bool {
        self.setting("save_audio").as_deref() != Some("0")
    }

    pub fn set_save_audio(&self, enabled: bool) -> Result<()> {
        self.set_setting("save_audio", if enabled { "1" } else { "0" })
    }

    pub fn mic_device(&self) -> String {
        self.setting("mic_device").unwrap_or_default()
    }

    pub fn set_mic_device(&self, name: &str) -> Result<()> {
        self.set_setting("mic_device", name)
    }

    pub fn transcribe_model(&self) -> String {
        self.setting("transcribe_model")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| crate::gateway::TRANSCRIBE_MODEL.to_string())
    }

    pub fn polish_model(&self) -> String {
        self.setting("polish_model")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| crate::gateway::POLISH_MODEL.to_string())
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn audio_path(&self, id: i64) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_default()
            .join("OpenTypeless")
            .join("audio")
            .join(format!("{id}.wav"))
    }

    pub fn save_audio_file(&self, id: i64, wav: &[u8]) -> Result<PathBuf> {
        let path = self.audio_path(id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, wav)?;
        Ok(path)
    }

    pub fn phrases(&self) -> Result<Vec<Phrase>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, phrase FROM phrases ORDER BY id DESC LIMIT 200")?;
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
            .take(150)
            .collect()
    }

    pub fn add_phrases_batch(&self, text: &str) -> Result<usize> {
        let mut added = 0usize;
        for token in text.split(|c| c == '\n' || c == ',' || c == '，' || c == ';' || c == '；') {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                let changed = self.conn.execute(
                    "INSERT OR IGNORE INTO phrases (phrase, created_at) VALUES (?1, ?2)",
                    params![trimmed, now()],
                )?;
                added += changed;
            }
        }
        Ok(added)
    }

    pub fn delete_phrase(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM phrases WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_history(
        &self,
        bundle_id: &str,
        transcript: &str,
        polished: &str,
        duration_ms: i64,
        mode: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO history (created_at, bundle_id, transcript, polished, duration_ms, mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![now(), bundle_id, transcript, polished, duration_ms, mode],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_history_polished(&self, id: i64, polished: &str, mode: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE history SET polished = ?1, mode = ?2 WHERE id = ?3",
            params![polished, mode, id],
        )?;
        Ok(())
    }

    pub fn delete_history(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM history WHERE id = ?1", params![id])?;
        let audio = self.audio_path(id);
        if audio.exists() {
            let _ = std::fs::remove_file(audio);
        }
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        let items = self.history().unwrap_or_default();
        for item in items {
            let audio = self.audio_path(item.id);
            if audio.exists() {
                let _ = std::fs::remove_file(audio);
            }
        }
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn history(&self) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, transcript, polished, bundle_id, duration_ms, mode
             FROM history ORDER BY id DESC LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                created_at: row.get(1)?,
                transcript: row.get(2)?,
                polished: row.get(3)?,
                bundle_id: row.get(4)?,
                duration_ms: row.get(5).unwrap_or(0),
                mode: row.get(6).unwrap_or_default(),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn stats(&self) -> StatsSummary {
        let items = self.history().unwrap_or_default();
        let total_sessions = items.len();
        let mut total_chars = 0usize;
        let mut total_duration_ms = 0i64;
        let mut timed_chars = 0usize;
        let mut timed_ms = 0i64;

        for item in &items {
            let chars = item.char_count();
            total_chars += chars;
            if item.duration_ms > 0 {
                total_duration_ms += item.duration_ms;
                timed_chars += chars;
                timed_ms += item.duration_ms;
            } else {
                // Estimate ~3.5 chars per second for legacy entries without duration_ms
                let estimated_ms = ((chars as f32 / 3.5) * 1000.0) as i64;
                total_duration_ms += estimated_ms;
            }
        }

        let avg_cpm = if timed_ms > 1000 {
            ((timed_chars as f64) / (timed_ms as f64 / 60_000.0)).round() as usize
        } else if total_duration_ms > 1000 {
            ((total_chars as f64) / (total_duration_ms as f64 / 60_000.0)).round() as usize
        } else {
            0
        };

        // Typical manual typing + editing speed on mixed CJK/English is ~45 chars/min
        let typing_minutes = total_chars as f32 / 45.0;
        let speaking_minutes = total_duration_ms as f32 / 60_000.0;
        let saved_minutes = (typing_minutes - speaking_minutes).max(0.0);

        StatsSummary {
            total_sessions,
            total_chars,
            total_duration_ms,
            avg_cpm,
            saved_minutes,
        }
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
