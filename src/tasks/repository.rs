use super::{Summary, Task, TaskConfig, CHUNK_SECS};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn root() -> Result<PathBuf, String> {
    let path = crate::store::Store::data_dir()
        .map_err(|_| "无法找到任务目录")?
        .join("tasks");
    fs::create_dir_all(&path).map_err(|_| "无法创建任务目录")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "无法设置任务目录权限")?;
    Ok(path)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|_| "无法写入任务文件，请检查磁盘空间")?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| "任务文件写入失败，请检查磁盘空间")?;
    fs::rename(&temp, path).map_err(|_| "无法完成任务文件保存")?;
    File::open(path.parent().ok_or("任务路径无效")?)
        .and_then(|d| d.sync_all())
        .map_err(|_| "无法同步任务目录".to_string())
}
impl Task {
    pub fn new(channels: u16, sample_rate: u32, config: TaskConfig) -> Result<Self, String> {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let task = Self {
            location: None,
            id: format!(
                "{timestamp}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ),
            created: now(),
            updated: now(),
            channels,
            sample_rate,
            config,
            phase: "录音中".into(),
            error: String::new(),
            transcripts: Vec::new(),
            transcript: String::new(),
            corrected: String::new(),
            polished: String::new(),
            polish_parts: Vec::new(),
            history_id: None,
        };
        fs::create_dir(task.dir()?).map_err(|_| "无法创建录音任务")?;
        fs::set_permissions(task.dir()?, fs::Permissions::from_mode(0o700))
            .map_err(|_| "无法设置录音权限")?;
        task.save()?;
        Ok(task)
    }
    pub fn dir(&self) -> Result<PathBuf, String> {
        if let Some(path) = &self.location {
            Ok(path.clone())
        } else {
            task_dir(&self.id)
        }
    }
    pub fn save(&self) -> Result<(), String> {
        let mut task = self.clone();
        task.updated = now();
        atomic_write(
            &self.dir()?.join("task.json"),
            &serde_json::to_vec(&task).map_err(|_| "无法编码任务状态")?,
        )
    }
    pub fn load(id: &str) -> Result<Self, String> {
        let bytes = fs::read(task_dir(id)?.join("task.json")).map_err(|_| "无法读取恢复任务")?;
        let task: Self = serde_json::from_slice(&bytes).map_err(|_| "恢复任务格式损坏")?;
        if task.id != id
            || task.channels == 0
            || task.channels > 32
            || task.sample_rate == 0
            || task.sample_rate > 384_000
        {
            return Err("恢复任务参数无效".into());
        }
        Ok(task)
    }
    pub fn available_bytes(&self) -> Result<u64, String> {
        let dir = self.dir()?;
        let checkpoint: u64 = fs::read_to_string(dir.join("checkpoint"))
            .unwrap_or_default()
            .parse()
            .unwrap_or(0);
        let actual = fs::metadata(dir.join("audio.raw"))
            .map_err(|_| "录音文件不存在")?
            .len();
        let frame = self.channels as u64 * 4;
        Ok(checkpoint.min(actual) / frame * frame)
    }
    pub fn duration_ms(&self) -> Result<i64, String> {
        Ok(
            (self.available_bytes()? * 1000 / (self.channels as u64 * 4 * self.sample_rate as u64))
                as i64,
        )
    }
    pub fn chunk_count(&self) -> Result<usize, String> {
        Ok((self.available_bytes()? as usize).div_ceil(self.chunk_bytes()))
    }
    fn chunk_bytes(&self) -> usize {
        self.sample_rate as usize * self.channels as usize * 4 * CHUNK_SECS
    }
    pub fn chunk_wav(&self, index: usize) -> Result<Vec<u8>, String> {
        let offset = index
            .checked_mul(self.chunk_bytes())
            .ok_or("音频片段越界")? as u64;
        let available = self.available_bytes()?;
        if offset >= available {
            return Err("录音为空或音频片段越界".into());
        }
        let len = (available - offset).min(self.chunk_bytes() as u64) as usize;
        let mut f = File::open(self.dir()?.join("audio.raw")).map_err(|_| "无法读取录音")?;
        f.seek(SeekFrom::Start(offset))
            .map_err(|_| "无法定位录音")?;
        let mut bytes = vec![0; len];
        f.read_exact(&mut bytes).map_err(|_| "录音读取不完整")?;
        let samples = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        Ok(crate::audio::Recorded {
            samples,
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
        .to_wav())
    }
    pub fn wav(&self) -> Result<Vec<u8>, String> {
        let mut pcm = Vec::new();
        for i in 0..self.chunk_count()? {
            let wav = self.chunk_wav(i)?;
            pcm.extend_from_slice(&wav[44..]);
        }
        Ok(crate::audio::encode_wav(&pcm, 16_000))
    }
    pub fn summary(&self) -> Summary {
        let transcript = if self.transcript.is_empty() {
            self.transcripts.join("\n")
        } else {
            self.transcript.clone()
        };
        Summary {
            id: self.id.clone(),
            phase: self.phase.clone(),
            error: self.error.clone(),
            transcript: transcript.clone(),
            output: if self.polished.is_empty() {
                if self.corrected.is_empty() {
                    transcript
                } else {
                    self.corrected.clone()
                }
            } else {
                self.polished.clone()
            },
        }
    }
    pub fn remove(&self) -> Result<(), String> {
        fs::remove_dir_all(self.dir()?).map_err(|_| "无法删除恢复任务".to_string())
    }
}
fn task_dir(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return Err("任务编号无效".into());
    }
    Ok(root()?.join(id))
}
pub fn remove_task(id: &str) -> Result<(), String> {
    fs::remove_dir_all(task_dir(id)?).map_err(|_| "无法删除恢复任务".into())
}
pub fn recover() -> Result<Vec<Summary>, String> {
    let mut tasks = Vec::new();
    for entry in fs::read_dir(root()?).map_err(|_| "无法扫描恢复任务")? {
        let entry = entry.map_err(|_| "无法读取任务目录")?;
        let id = entry.file_name().to_string_lossy().to_string();
        if let Ok(mut task) = Task::load(&id) {
            if task.phase == "已完成" && task.polished.is_empty() && task.config.options.is_some()
            {
                task.phase = "润色待重试".into();
                task.save()?;
            }
            if task.phase == "已完成" {
                continue;
            }
            if now().saturating_sub(task.updated) > 7 * 86400 {
                task.remove()?;
                continue;
            }
            if matches!(task.phase.as_str(), "录音中" | "转写中" | "润色中") {
                task.phase = "已中断，可恢复".into();
            }
            tasks.push(task.summary());
        } else if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            tasks.push(Summary {
                id,
                phase: "任务元数据损坏".into(),
                error: "原始文件仍保留在本机 tasks 目录；请备份后处理".into(),
                transcript: String::new(),
                output: String::new(),
            });
        }
    }
    tasks.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(tasks)
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::providers::Profile;
    use crate::tasks::{AudioWriter, TaskConfig};
    pub(crate) fn fixture() -> Task {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "notype-task-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let store = crate::store::Store::open_in_memory().unwrap();
        let task = Task {
            location: Some(dir),
            id: "123-456".into(),
            created: now(),
            updated: now(),
            channels: 1,
            sample_rate: 16000,
            config: TaskConfig {
                transcribe: Profile::load(&store, crate::providers::ProviderKind::Vercel),
                polish: Profile::load(&store, crate::providers::ProviderKind::OpenRouter),
                options: None,
                bundle: "test.app".into(),
                mode: "raw".into(),
                save_audio: false,
            },
            phase: "录音中".into(),
            error: String::new(),
            transcripts: Vec::new(),
            transcript: String::new(),
            corrected: String::new(),
            polished: String::new(),
            polish_parts: Vec::new(),
            history_id: None,
        };
        task.save().unwrap();
        task
    }
    #[test]
    fn journal_finalizes_and_preserves_only_complete_frames() {
        let task = fixture();
        let writer = AudioWriter::start(&task).unwrap();
        let sender = writer.sender();
        sender.send(vec![0.25; 1600]).unwrap();
        drop(sender);
        let task = writer.finish(task).finish().unwrap();
        assert_eq!(task.duration_ms().unwrap(), 100);
        assert_eq!(task.chunk_count().unwrap(), 1);
        assert_eq!(task.chunk_wav(0).unwrap().len(), 3244);
        assert_eq!(task.wav().unwrap().len(), 3244);
        let dir = task.dir().unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(dir.join("audio.raw"))
            .unwrap();
        f.write_all(&[0; 17]).unwrap();
        assert_eq!(task.available_bytes().unwrap(), 6400); // Unsynced tail is never consumed.
        task.remove().unwrap();
    }
    #[test]
    fn ten_minutes_of_audio_is_bounded_into_twenty_complete_chunks() {
        let task = fixture();
        let writer = AudioWriter::start(&task).unwrap();
        let sender = writer.sender();
        for _ in 0..600 {
            sender.send(vec![0.1; 16000]).unwrap();
        }
        drop(sender);
        let task = writer.finish(task).finish().unwrap();
        assert_eq!(task.duration_ms().unwrap(), 600000);
        assert_eq!(task.chunk_count().unwrap(), 20);
        let mut bytes = 0;
        for index in 0..20 {
            bytes += task.chunk_wav(index).unwrap().len() - 44;
        }
        assert_eq!(bytes, 19_200_000);
        task.remove().unwrap();
    }
    #[test]
    fn corrupt_checkpoint_and_empty_audio_are_recoverable_errors() {
        let task = fixture();
        fs::write(task.dir().unwrap().join("audio.raw"), [0; 8]).unwrap();
        fs::write(task.dir().unwrap().join("checkpoint"), "broken").unwrap();
        assert_eq!(task.available_bytes().unwrap(), 0);
        assert!(task.chunk_wav(0).is_err());
        task.remove().unwrap();
    }
    #[test]
    fn history_retry_is_idempotent_and_updates_polish() {
        let mut task = fixture();
        task.transcript = "synthetic text".into();
        let store = crate::store::Store::open_in_memory().unwrap();
        let first = store.save_task_history(&task).unwrap();
        task.polished = "polished synthetic text".into();
        let second = store.save_task_history(&task).unwrap();
        assert_eq!(first, second);
        let history = store.history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].polished, "polished synthetic text");
        task.remove().unwrap();
    }
    #[test]
    fn manifest_round_trip_does_not_store_filesystem_override_or_credentials() {
        let task = fixture();
        let bytes = fs::read(task.dir().unwrap().join("task.json")).unwrap();
        let restored: Task = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.id, task.id);
        assert!(restored.location.is_none());
        assert_eq!(
            restored.config.polish.kind,
            crate::providers::ProviderKind::OpenRouter
        );
        task.remove().unwrap();
    }
    #[test]
    fn rejects_path_traversal() {
        assert!(task_dir("../audio").is_err());
        assert!(task_dir("").is_err());
    }
}
