use crate::polish::PolishOptions;
use crate::providers::Profile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub const MAX_RECORDING_SECS: u64 = 600;
pub const CHUNK_SECS: usize = 30;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskConfig {
    pub transcribe: Profile,
    pub polish: Profile,
    pub options: Option<PolishOptions>,
    pub bundle: String,
    pub mode: String,
    pub save_audio: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    #[serde(skip)]
    pub(super) location: Option<PathBuf>,
    pub id: String,
    pub created: u64,
    pub updated: u64,
    pub channels: u16,
    pub sample_rate: u32,
    pub config: TaskConfig,
    pub phase: String,
    pub error: String,
    pub transcripts: Vec<String>,
    pub transcript: String,
    #[serde(default)]
    pub corrected: String,
    pub polished: String,
    #[serde(default)]
    pub polish_parts: Vec<String>,
    pub history_id: Option<i64>,
}
#[derive(Clone, Debug)]
pub struct Summary {
    pub id: String,
    pub phase: String,
    pub error: String,
    pub transcript: String,
    pub output: String,
}
