use super::{atomic_write, Task};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::{
    mpsc::{self, SyncSender},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

pub struct AudioWriter {
    sender: SyncSender<Vec<f32>>,
    pub error: Arc<Mutex<Option<String>>>,
    worker: std::thread::JoinHandle<Result<(), String>>,
}
pub struct CaptureResult {
    pub task: Task,
    worker: std::thread::JoinHandle<Result<(), String>>,
}
impl AudioWriter {
    pub fn start(task: &Task) -> Result<Self, String> {
        let dir = task.dir()?;
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(dir.join("audio.raw"))
            .map_err(|_| "无法创建录音文件")?;
        let (sender, receiver) = mpsc::sync_channel::<Vec<f32>>(64);
        let error = Arc::new(Mutex::new(None));
        let errors = error.clone();
        let worker = std::thread::spawn(move || {
            let result = (|| {
                let mut file: File = file;
                let mut bytes_written = 0u64;
                let mut checkpoint = Instant::now();
                while let Ok(samples) = receiver.recv() {
                    let mut bytes = Vec::with_capacity(samples.len() * 4);
                    for sample in samples {
                        bytes.extend_from_slice(&sample.to_le_bytes());
                    }
                    file.write_all(&bytes)
                        .map_err(|_| "录音写入失败，请检查磁盘空间")?;
                    bytes_written += bytes.len() as u64;
                    if checkpoint.elapsed() >= Duration::from_secs(2) {
                        file.sync_data().map_err(|_| "录音同步失败")?;
                        atomic_write(
                            &dir.join("checkpoint"),
                            bytes_written.to_string().as_bytes(),
                        )?;
                        checkpoint = Instant::now();
                    }
                }
                file.sync_all().map_err(|_| "录音保存失败")?;
                atomic_write(
                    &dir.join("checkpoint"),
                    bytes_written.to_string().as_bytes(),
                )
            })();
            if let Err(ref message) = result {
                if let Ok(mut e) = errors.lock() {
                    *e = Some(message.clone());
                }
            }
            result
        });
        Ok(Self {
            sender,
            error,
            worker,
        })
    }
    pub fn sender(&self) -> SyncSender<Vec<f32>> {
        self.sender.clone()
    }
    pub fn finish(self, task: Task) -> CaptureResult {
        drop(self.sender);
        CaptureResult {
            task,
            worker: self.worker,
        }
    }
}
impl CaptureResult {
    /// Joining and durable writes must run on the background executor.
    pub fn finish(self) -> Result<Task, String> {
        self.worker.join().map_err(|_| "录音写入线程异常")??;
        let mut task = self.task;
        task.phase = "已保存".into();
        task.save()?;
        Ok(task)
    }
}
