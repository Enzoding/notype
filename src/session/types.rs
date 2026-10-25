use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Transcribe,
    Polish,
    Insert,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Transcribe => "正在转写语音",
            Stage::Polish => "正在智能润色",
            Stage::Insert => "正在贴入窗口",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Stage::Transcribe => "转写",
            Stage::Polish => "润色",
            Stage::Insert => "贴入",
        }
    }

    #[allow(dead_code)]
    pub fn index(self) -> usize {
        match self {
            Stage::Transcribe => 0,
            Stage::Polish => 1,
            Stage::Insert => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Done(String),
    Copied,
    Warn(String),
    Error(String),
}

impl Notice {
    pub fn text(&self) -> &str {
        match self {
            Notice::Done(text) => text,
            Notice::Copied => "已复制，请手动粘贴",
            Notice::Warn(text) | Notice::Error(text) => text,
        }
    }

    pub(super) fn duration(&self) -> Duration {
        match self {
            Notice::Error(_) | Notice::Warn(_) => Duration::from_millis(3600),
            _ => Duration::from_millis(2200),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Listening,
    Working(Stage),
    Message(Notice),
}
