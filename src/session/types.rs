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
            Stage::Transcribe => crate::i18n::t("stage.transcribe"),
            Stage::Polish => crate::i18n::t("stage.polish"),
            Stage::Insert => crate::i18n::t("stage.insert"),
        }
    }

    #[allow(dead_code)]
    pub fn short_label(self) -> &'static str {
        match self {
            Stage::Transcribe => crate::i18n::t("stage.transcribe_short"),
            Stage::Polish => crate::i18n::t("stage.polish_short"),
            Stage::Insert => crate::i18n::t("stage.insert_short"),
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
            Notice::Copied => crate::i18n::t("notice.copied"),
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
