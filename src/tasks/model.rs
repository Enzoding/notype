use crate::polish::PolishOptions;
use crate::providers::Profile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_RECORDING_SECS: u64 = 600;
pub const CHUNK_SECS: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskPhase {
    Recording,
    Saved,
    Transcribing,
    Transcribed,
    Polishing,
    PolishRetryPending,
    ReadyForDelivery,
    Completed,
    Cancelled,
    Interrupted,
    AudioSaveError,
    TooShort,
}

impl TaskPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Saved => "saved",
            Self::Transcribing => "transcribing",
            Self::Transcribed => "transcribed",
            Self::Polishing => "polishing",
            Self::PolishRetryPending => "polish_retry_pending",
            Self::ReadyForDelivery => "ready_for_delivery",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::AudioSaveError => "audio_save_error",
            Self::TooShort => "too_short",
        }
    }

    pub fn from_str(raw: &str) -> Self {
        match raw.trim() {
            "recording" | "录音中" => Self::Recording,
            "saved" | "已保存" => Self::Saved,
            "transcribing" | "转写中" => Self::Transcribing,
            "transcribed" | "已转写" => Self::Transcribed,
            "polishing" | "润色中" => Self::Polishing,
            "polish_retry_pending" | "润色待重试" => Self::PolishRetryPending,
            "ready_for_delivery" | "待交付" => Self::ReadyForDelivery,
            "completed" | "已完成" => Self::Completed,
            "cancelled" | "已取消" => Self::Cancelled,
            "interrupted" | "已中断，可恢复" => Self::Interrupted,
            "audio_save_error" | "录音保存异常" => Self::AudioSaveError,
            "too_short" | "录音过短" => Self::TooShort,
            _ => Self::Interrupted,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Recording => crate::i18n::t("task.phase.recording"),
            Self::Saved => crate::i18n::t("task.phase.saved"),
            Self::Transcribing => crate::i18n::t("task.phase.transcribing"),
            Self::Transcribed => crate::i18n::t("task.phase.transcribed"),
            Self::Polishing => crate::i18n::t("task.phase.polishing"),
            Self::PolishRetryPending => crate::i18n::t("task.phase.polish_retry_pending"),
            Self::ReadyForDelivery => crate::i18n::t("task.phase.ready_for_delivery"),
            Self::Completed => crate::i18n::t("task.phase.completed"),
            Self::Cancelled => crate::i18n::t("task.phase.cancelled"),
            Self::Interrupted => crate::i18n::t("task.phase.interrupted"),
            Self::AudioSaveError => crate::i18n::t("task.phase.audio_save_error"),
            Self::TooShort => crate::i18n::t("task.phase.too_short"),
        }
    }
}

impl std::fmt::Display for TaskPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for TaskPhase {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TaskPhase {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_str(&s))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskConfig {
    pub transcribe: Profile,
    pub polish: Profile,
    pub options: Option<PolishOptions>,
    pub bundle: String,
    pub mode: String,
    pub save_audio: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskTimings {
    #[serde(default)]
    pub asr_ms: u64,
    #[serde(default)]
    pub clean_ms: u64,
    #[serde(default)]
    pub polish_ms: u64,
    #[serde(default)]
    pub delivery_ms: u64,
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
    pub phase: TaskPhase,
    pub error: String,
    pub transcripts: Vec<String>,
    pub transcript: String,
    #[serde(default)]
    pub corrected: String,
    pub polished: String,
    #[serde(default)]
    pub polish_parts: Vec<String>,
    pub history_id: Option<i64>,
    #[serde(default)]
    pub timings: TaskTimings,
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub id: String,
    pub phase: TaskPhase,
    pub error: String,
    pub transcript: String,
    pub output: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_phase_serde_and_legacy_compatibility() {
        // Canonical ascii round-trip
        let serialized = serde_json::to_string(&TaskPhase::Recording).unwrap();
        assert_eq!(serialized, "\"recording\"");
        let deserialized: TaskPhase = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TaskPhase::Recording);

        // Legacy Chinese strings
        let legacy_cases = [
            ("\"录音中\"", TaskPhase::Recording),
            ("\"已保存\"", TaskPhase::Saved),
            ("\"转写中\"", TaskPhase::Transcribing),
            ("\"已转写\"", TaskPhase::Transcribed),
            ("\"润色中\"", TaskPhase::Polishing),
            ("\"润色待重试\"", TaskPhase::PolishRetryPending),
            ("\"待交付\"", TaskPhase::ReadyForDelivery),
            ("\"已完成\"", TaskPhase::Completed),
            ("\"已取消\"", TaskPhase::Cancelled),
            ("\"已中断，可恢复\"", TaskPhase::Interrupted),
            ("\"录音保存异常\"", TaskPhase::AudioSaveError),
            ("\"录音过短\"", TaskPhase::TooShort),
        ];

        for (json, expected) in legacy_cases {
            let phase: TaskPhase = serde_json::from_str(json).unwrap();
            assert_eq!(phase, expected, "Failed for legacy JSON: {}", json);
        }

        // Unknown defaults to Interrupted
        let fallback: TaskPhase = serde_json::from_str("\"未知状态\"").unwrap();
        assert_eq!(fallback, TaskPhase::Interrupted);
    }
}
