use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::mining::{
    ACTIVE_PHRASE_LIMIT, CandidateDraft, CorrectionRule, Evidence, normalize_term, term_key,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    pub id: i64,
    pub phrase: String,
    pub source: String,
    pub hits: i64,
    pub last_seen: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhraseCandidate {
    pub id: i64,
    pub phrase: String,
    pub kind: String,
    pub source: String,
    pub confidence: f32,
    pub score: f32,
    pub hits: i64,
    pub sessions: i64,
    pub apps: i64,
    pub evidence: Vec<Evidence>,
    pub status: String,
    pub first_seen: i64,
    pub last_seen: i64,
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
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(2))?;
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
            CREATE TABLE IF NOT EXISTS phrase_candidates (
                id INTEGER PRIMARY KEY,
                phrase TEXT NOT NULL,
                phrase_key TEXT NOT NULL UNIQUE,
                kind TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'llm',
                confidence REAL NOT NULL DEFAULT 0,
                score REAL NOT NULL DEFAULT 0,
                hits INTEGER NOT NULL DEFAULT 1,
                sessions INTEGER NOT NULL DEFAULT 1,
                apps INTEGER NOT NULL DEFAULT 1,
                evidence TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'pending',
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS corrections (
                id INTEGER PRIMARY KEY,
                wrong TEXT NOT NULL,
                right TEXT NOT NULL,
                hits INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL DEFAULT 'auto',
                created_at INTEGER NOT NULL,
                UNIQUE(wrong, right)
            );
            CREATE INDEX IF NOT EXISTS idx_candidates_status
                ON phrase_candidates(status, score DESC);
            ",
        )?;
        // Seamless schema migration for existing databases.
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN mode TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE phrases ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'",
            [],
        );
        let _ = conn.execute("ALTER TABLE phrases ADD COLUMN phrase_key TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE phrases ADD COLUMN hits INTEGER NOT NULL DEFAULT 1",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE phrases ADD COLUMN last_seen INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "UPDATE phrases SET phrase_key = lower(phrase)
             WHERE phrase_key IS NULL OR phrase_key = ''",
            [],
        );
        let _ = conn.execute(
            "UPDATE phrases SET last_seen = created_at WHERE last_seen = 0",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_phrases_last_seen ON phrases(last_seen DESC)",
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

    pub fn auto_learn_mode(&self) -> String {
        self.setting("auto_learn_mode")
            .unwrap_or_else(|| "auto".to_string())
    }

    pub fn set_auto_learn_mode(&self, mode: &str) -> Result<()> {
        self.set_setting("auto_learn_mode", mode)
    }

    pub fn learn_from_edits(&self) -> bool {
        self.setting("learn_from_edits").as_deref() != Some("0")
    }

    pub fn set_learn_from_edits(&self, enabled: bool) -> Result<()> {
        self.set_setting("learn_from_edits", if enabled { "1" } else { "0" })
    }

    pub fn last_mined_history_id(&self) -> i64 {
        self.setting("last_mined_history_id")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
    }

    pub fn set_last_mined_history_id(&self, id: i64) -> Result<()> {
        self.set_setting("last_mined_history_id", &id.to_string())
    }

    pub fn last_mined_at(&self) -> i64 {
        self.setting("last_mined_at")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
    }

    pub fn set_last_mined_at(&self, at: i64) -> Result<()> {
        self.set_setting("last_mined_at", &at.to_string())
    }

    pub fn last_mine_error(&self) -> String {
        self.setting("last_mine_error").unwrap_or_default()
    }

    pub fn set_last_mine_error(&self, error: &str) -> Result<()> {
        self.set_setting("last_mine_error", error)
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

    pub fn audio_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("audio"))
    }

    pub fn existing_audio_ids() -> HashSet<i64> {
        let mut ids = HashSet::new();
        if let Ok(dir) = Self::audio_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if let Some(stem) = name.strip_suffix(".wav") {
                            if let Ok(id) = stem.parse::<i64>() {
                                ids.insert(id);
                            }
                        }
                    }
                }
            }
        }
        ids
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
        let mut stmt = self.conn.prepare(
            "SELECT id, phrase, COALESCE(source, 'manual'), COALESCE(hits, 1),
                    COALESCE(last_seen, 0)
             FROM phrases ORDER BY id DESC LIMIT 1000",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Phrase {
                id: row.get(0)?,
                phrase: row.get(1)?,
                source: row.get(2)?,
                hits: row.get(3)?,
                last_seen: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn phrase_keys(&self) -> Vec<String> {
        let mut stmt = match self
            .conn
            .prepare("SELECT phrase_key FROM phrases WHERE phrase_key IS NOT NULL")
        {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], |row| row.get::<_, String>(0))
            .map(|rows| rows.filter_map(|row| row.ok()).collect())
            .unwrap_or_default()
    }

    pub fn phrase_list(&self) -> Vec<String> {
        self.phrases_ranked(ACTIVE_PHRASE_LIMIT)
            .into_iter()
            .map(|item| item.phrase)
            .collect()
    }

    /// Active list sent to ASR and the polish glossary. Manual and user-accepted
    /// entries come first, then the most frequently seen automatic entries.
    pub fn phrases_ranked(&self, limit: usize) -> Vec<Phrase> {
        let mut items = self.phrases().unwrap_or_default();
        let rank = |source: &str| match source {
            "manual" => 0,
            "accepted" => 1,
            "preset" => 2,
            "edit" => 3,
            _ => 4,
        };
        items.sort_by(|a, b| {
            rank(&a.source)
                .cmp(&rank(&b.source))
                .then(b.hits.cmp(&a.hits))
                .then(b.last_seen.cmp(&a.last_seen))
                .then(b.id.cmp(&a.id))
        });
        items.truncate(limit);
        items
    }

    pub fn add_phrases_batch(&self, text: &str) -> Result<usize> {
        let mut added = 0usize;
        for token in text.split(|c| {
            c == '\n' || c == ',' || c == '，' || c == ';' || c == '；'
        }) {
            let Some(term) = normalize_term(token) else {
                continue;
            };
            let len = term.chars().count();
            if term.contains('。') || term.contains('！') || term.contains('？') {
                continue;
            }
            if term.chars().all(crate::mining::is_cjk_char) && len > 12 {
                continue;
            }
            let key = term_key(&term);
            if self.phrase_key_exists(&key) {
                continue;
            }
            let changed = self.conn.execute(
                "INSERT OR IGNORE INTO phrases
                    (phrase, phrase_key, source, hits, last_seen, created_at)
                 VALUES (?1, ?2, 'manual', 1, ?3, ?3)",
                params![term, key, now()],
            )?;
            added += changed;
        }
        Ok(added)
    }

    fn phrase_key_exists(&self, key: &str) -> bool {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM phrases WHERE phrase_key = ?1)",
                params![key],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .unwrap_or(false)
    }

    fn insert_phrase(&self, phrase: &str, source: &str, hits: i64, last_seen: i64) -> Result<()> {
        let key = term_key(phrase);
        let stamp = if last_seen > 0 { last_seen } else { now() };
        let updated = self.conn.execute(
            "UPDATE phrases
             SET hits = MAX(hits, ?1),
                 last_seen = MAX(last_seen, ?2),
                 source = CASE WHEN source = 'manual' THEN 'manual' ELSE ?3 END
             WHERE phrase_key = ?4",
            params![hits, stamp, source, key],
        )?;
        if updated == 0 {
            self.conn.execute(
                "INSERT INTO phrases
                    (phrase, phrase_key, source, hits, last_seen, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![phrase, key, source, hits.max(1), stamp],
            )?;
        }
        Ok(())
    }

    pub fn delete_phrase(&self, id: i64) -> Result<()> {
        let phrase: Option<String> = self
            .conn
            .query_row(
                "SELECT phrase FROM phrases WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();
        self.conn
            .execute("DELETE FROM phrases WHERE id = ?1", params![id])?;
        if let Some(phrase) = phrase {
            // Deleting is a strong negative signal: never auto-suggest it again.
            let _ = self.conn.execute(
                "DELETE FROM corrections WHERE lower(right) = lower(?1)",
                params![phrase],
            );
            self.remember_ignored(&phrase, "deleted")?;
        }
        Ok(())
    }

    pub fn upsert_candidates(&self, drafts: &[CandidateDraft]) -> Result<()> {
        let stamp = now();
        for draft in drafts {
            let evidence = serde_json::to_string(&draft.evidence).unwrap_or_else(|_| "[]".into());
            let updated = self.conn.execute(
                "UPDATE phrase_candidates SET
                    phrase = CASE WHEN length(?2) > length(phrase) THEN ?2 ELSE phrase END,
                    kind = CASE WHEN ?3 <> '' THEN ?3 ELSE kind END,
                    source = CASE WHEN source IN ('edit', 'manual') THEN source ELSE ?4 END,
                    confidence = MAX(confidence, ?5),
                    score = MAX(score, ?6),
                    hits = hits + 1,
                    sessions = MAX(sessions, ?7),
                    apps = MAX(apps, ?8),
                    evidence = ?9,
                    last_seen = ?10
                 WHERE phrase_key = ?1",
                params![
                    draft.phrase_key,
                    draft.phrase,
                    draft.kind.as_str(),
                    draft.source.as_str(),
                    draft.confidence,
                    draft.score,
                    draft.sessions as i64,
                    draft.apps as i64,
                    evidence,
                    stamp
                ],
            )?;
            if updated == 0 {
                self.conn.execute(
                    "INSERT INTO phrase_candidates
                        (phrase, phrase_key, kind, source, confidence, score, hits, sessions,
                         apps, evidence, status, first_seen, last_seen)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, 'pending', ?10, ?10)",
                    params![
                        draft.phrase,
                        draft.phrase_key,
                        draft.kind.as_str(),
                        draft.source.as_str(),
                        draft.confidence,
                        draft.score,
                        draft.sessions as i64,
                        draft.apps as i64,
                        evidence,
                        stamp
                    ],
                )?;
            } else {
                let status: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT status FROM phrase_candidates WHERE phrase_key = ?1",
                        params![draft.phrase_key],
                        |row| row.get(0),
                    )
                    .ok();
                if status.as_deref() == Some("accepted") {
                    let _ = self.insert_phrase(
                        &draft.phrase,
                        "accepted",
                        draft.sessions as i64,
                        stamp,
                    );
                }
            }
        }
        Ok(())
    }

    pub fn pending_candidates(&self) -> Result<Vec<PhraseCandidate>> {
        self.candidates(Some("pending"))
    }

    pub fn candidates(&self, status: Option<&str>) -> Result<Vec<PhraseCandidate>> {
        let (sql, filter) = match status {
            Some(status) => (
                "SELECT id, phrase, kind, source, confidence, score, hits, sessions, apps,
                        evidence, status, first_seen, last_seen
                 FROM phrase_candidates WHERE status = ?1
                 ORDER BY score DESC, last_seen DESC",
                Some(status),
            ),
            None => (
                "SELECT id, phrase, kind, source, confidence, score, hits, sessions, apps,
                        evidence, status, first_seen, last_seen
                 FROM phrase_candidates ORDER BY score DESC, last_seen DESC",
                None,
            ),
        };
        let mut stmt = self.conn.prepare(sql)?;
        let map = |row: &rusqlite::Row<'_>| {
            let evidence_json: String = row.get(9).unwrap_or_else(|_| "[]".into());
            let evidence: Vec<Evidence> =
                serde_json::from_str(&evidence_json).unwrap_or_default();
            Ok(PhraseCandidate {
                id: row.get(0)?,
                phrase: row.get(1)?,
                kind: row.get(2)?,
                source: row.get(3)?,
                confidence: row.get::<_, f64>(4)? as f32,
                score: row.get::<_, f64>(5)? as f32,
                hits: row.get(6)?,
                sessions: row.get(7)?,
                apps: row.get(8)?,
                evidence,
                status: row.get(10)?,
                first_seen: row.get(11)?,
                last_seen: row.get(12)?,
            })
        };
        let rows = match filter {
            Some(status) => stmt.query_map(params![status], map)?,
            None => stmt.query_map([], map)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn accept_candidate(&self, id: i64) -> Result<Option<String>> {
        let row: Option<(String, i64, i64)> = self
            .conn
            .query_row(
                "SELECT phrase, hits, last_seen FROM phrase_candidates WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();
        let Some((phrase, hits, last_seen)) = row else {
            return Ok(None);
        };
        self.conn.execute(
            "UPDATE phrase_candidates SET status = 'accepted' WHERE id = ?1",
            params![id],
        )?;
        self.insert_phrase(&phrase, "accepted", hits, last_seen)?;
        Ok(Some(phrase))
    }

    pub fn accept_candidate_by_key(&self, key: &str) -> Result<Option<String>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM phrase_candidates WHERE phrase_key = ?1 AND status = 'pending'",
                params![key],
                |row| row.get(0),
            )
            .ok();
        match id {
            Some(id) => self.accept_candidate(id),
            None => Ok(None),
        }
    }

    pub fn ignore_candidate(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE phrase_candidates SET status = 'ignored' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn accept_all_pending(&self) -> Result<usize> {
        let pending = self.pending_candidates().unwrap_or_default();
        let mut accepted = 0usize;
        for item in pending {
            if self.accept_candidate(item.id)?.is_some() {
                accepted += 1;
            }
        }
        Ok(accepted)
    }

    pub fn remember_ignored(&self, phrase: &str, source: &str) -> Result<()> {
        let Some(phrase) = normalize_term(phrase) else {
            return Ok(());
        };
        let key = term_key(&phrase);
        let stamp = now();
        self.conn.execute(
            "INSERT INTO phrase_candidates
                (phrase, phrase_key, kind, source, confidence, score, hits, sessions, apps,
                 evidence, status, first_seen, last_seen)
             VALUES (?1, ?2, 'unknown', ?3, 0, 0, 1, 1, 1, '[]', 'ignored', ?4, ?4)
             ON CONFLICT(phrase_key) DO UPDATE SET
                status = 'ignored',
                last_seen = excluded.last_seen",
            params![phrase, key, source, stamp],
        )?;
        Ok(())
    }

    pub fn corrections(&self) -> Vec<CorrectionRule> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT wrong, right FROM corrections
             ORDER BY hits DESC, length(wrong) DESC LIMIT 200",
        ) else {
            return Vec::new();
        };
        stmt.query_map([], |row| {
            Ok(CorrectionRule {
                wrong: row.get(0)?,
                right: row.get(1)?,
            })
        })
        .map(|rows| rows.filter_map(|row| row.ok()).collect())
        .unwrap_or_default()
    }

    pub fn upsert_correction(&self, wrong: &str, right: &str) -> Result<()> {
        let Some(wrong) = normalize_term(wrong) else {
            return Ok(());
        };
        let Some(right) = normalize_term(right) else {
            return Ok(());
        };
        if wrong == right {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO corrections (wrong, right, hits, source, created_at)
             VALUES (?1, ?2, 1, 'auto', ?3)
             ON CONFLICT(wrong, right) DO UPDATE SET hits = hits + 1",
            params![wrong, right, now()],
        )?;
        Ok(())
    }

    pub fn delete_correction(&self, wrong: &str, right: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM corrections WHERE wrong = ?1 AND right = ?2",
            params![wrong, right],
        )?;
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

    pub fn history_item(&self, id: i64) -> Option<HistoryItem> {
        self.conn
            .query_row(
                "SELECT id, created_at, transcript, polished, bundle_id, duration_ms, mode
                 FROM history WHERE id = ?1",
                params![id],
                |row| {
                    Ok(HistoryItem {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        transcript: row.get(2)?,
                        polished: row.get(3)?,
                        bundle_id: row.get(4)?,
                        duration_ms: row.get(5).unwrap_or(0),
                        mode: row.get(6).unwrap_or_default(),
                    })
                },
            )
            .ok()
    }

    pub fn history_for_mining(&self, limit: usize) -> Result<Vec<HistoryItem>> {
        let mut items = self.history_since(0, limit)?;
        items.sort_by_key(|item| item.id);
        Ok(items)
    }

    pub fn history_since(&self, after_id: i64, limit: usize) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, transcript, polished, bundle_id, duration_ms, mode
             FROM history
             WHERE id > ?1 AND (polished <> '' OR transcript <> '')
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_id, limit as i64], |row| {
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

    pub fn count_history_after(&self, after_id: i64) -> usize {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM history WHERE id > ?1 AND (polished <> '' OR transcript <> '')",
                params![after_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
            .unwrap_or(0)
    }

    pub fn max_history_id(&self) -> i64 {
        self.conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM history", [], |row| {
                row.get(0)
            })
            .unwrap_or(0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mining::{CandidateSource, TermKind};

    fn draft(phrase: &str) -> CandidateDraft {
        let key = term_key(phrase);
        CandidateDraft {
            phrase: phrase.into(),
            phrase_key: key,
            kind: TermKind::Product,
            source: CandidateSource::Llm,
            confidence: 0.9,
            score: 0.9,
            sessions: 1,
            apps: 1,
            evidence: vec![Evidence {
                history_id: 1,
                quote: format!("关于{phrase}"),
                heard_as: None,
            }],
            auto_accept: true,
        }
    }

    #[test]
    fn candidate_accept_and_delete_are_remembered() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_candidates(&[draft("豆卡")]).unwrap();
        let pending = store.pending_candidates().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].evidence.len(), 1);

        let accepted = store.accept_candidate(pending[0].id).unwrap();
        assert_eq!(accepted.as_deref(), Some("豆卡"));
        assert_eq!(store.phrases().unwrap().len(), 1);
        assert!(store.pending_candidates().unwrap().is_empty());

        let id = store.phrases().unwrap()[0].id;
        store.delete_phrase(id).unwrap();
        assert!(store.phrases().unwrap().is_empty());

        // Re-mining the same term must not bring it back.
        store.upsert_candidates(&[draft("豆卡")]).unwrap();
        assert!(store.pending_candidates().unwrap().is_empty());
        assert!(store.accept_candidate_by_key("豆卡").unwrap().is_none());
    }

    #[test]
    fn ranking_keeps_newest_phrases_within_the_active_limit() {
        let store = Store::open_in_memory().unwrap();
        let terms: Vec<String> = (0..200).map(|index| format!("term{index:03}")).collect();
        store.add_phrases_batch(&terms.join(",")).unwrap();
        let ranked = store.phrases_ranked(ACTIVE_PHRASE_LIMIT);
        assert_eq!(ranked.len(), ACTIVE_PHRASE_LIMIT);
        assert!(ranked.iter().any(|item| item.phrase == "term199"));
        assert!(!ranked.iter().any(|item| item.phrase == "term000"));
    }

    #[test]
    fn raw_only_history_is_still_mined() {
        let store = Store::open_in_memory().unwrap();
        store
            .add_history("com.apple.Notes", "豆卡很好用", "", 1000, "raw")
            .unwrap();
        assert_eq!(store.count_history_after(0), 1);
        assert_eq!(store.history_for_mining(10).unwrap().len(), 1);
    }

    #[test]
    fn deleting_a_phrase_removes_its_correction() {
        let store = Store::open_in_memory().unwrap();
        store.add_phrases_batch("豆卡").unwrap();
        store.upsert_correction("斗卡", "豆卡").unwrap();
        assert_eq!(store.corrections().len(), 1);
        let id = store.phrases().unwrap()[0].id;
        store.delete_phrase(id).unwrap();
        assert!(store.corrections().is_empty());
    }

    #[test]
    fn audio_dir_resolves_and_ids_scan_gracefully() {
        let dir = Store::audio_dir().unwrap();
        assert!(dir.ends_with("audio"));
        let _ = Store::existing_audio_ids();
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
