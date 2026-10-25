use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use gpui_kit::Context;

use crate::capture::ActiveRecording;
use crate::gateway::{POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::mining::{AutoLearnMode, CorrectionRule};
use crate::polish::PolishMode;
use crate::secrets;
use crate::store::{
    HistoryItem, Phrase, PhraseCandidate, StatsSummary, Store, TriggerMode,
};

mod history;
mod lifecycle;
mod mining;
mod settings;
mod studio;
mod time;
mod types;

pub use time::{format_duration_ms, format_relative_time, mmss};
pub use types::{Notice, Phase, Stage};

pub struct Session {
    pub phase: Phase,
    pub level: f32,
    pub wave: [f32; crate::capture::WAVE_BARS],
    pub show_dock: bool,
    pub hotkey_code: i64,
    pub trigger_mode: TriggerMode,
    pub polish_strategy: String,
    pub custom_prompt: String,
    pub space_cjk: bool,
    pub sound_feedback: bool,
    pub save_audio: bool,
    pub mic_device: String,
    pub available_mics: Vec<String>,
    pub transcribe_model: String,
    pub polish_model: String,
    pub key_status: String,
    pub gateway_test_status: Option<(bool, String)>,
    pub gateway_testing: bool,
    pub phrases: Vec<Phrase>,
    pub candidates: Vec<PhraseCandidate>,
    pub corrections: Vec<CorrectionRule>,
    pub auto_learn_mode: AutoLearnMode,
    pub learn_from_edits: bool,
    pub mining: bool,
    pub mining_status: String,
    pub last_mined_at: i64,
    pub audio_ids: HashSet<i64>,
    pub history: Vec<HistoryItem>,
    pub stats: StatsSummary,
    pub active_bundle: String,
    pub playing_id: Option<i64>,
    pub repolishing_id: Option<i64>,
    pub studio_mode: PolishMode,
    pub studio_output: String,
    pub studio_raw: String,
    pub studio_busy: bool,
    pub studio_dictating: bool,
    pub mic_testing: bool,
    pub(super) key: String,
    pub(super) store: Arc<Mutex<Store>>,
    pub(super) recording: Option<ActiveRecording>,
    pub(super) mic_test_stream: Option<ActiveRecording>,
    pub(super) stages: Option<Receiver<Stage>>,
    pub(super) generation: u64,
    pub(super) note_id: u64,
    pub appearance: u64,
    pub exiting: bool,
    pub(super) hide_epoch: u64,
}

impl Session {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let store = Store::open().expect("打开本机数据库");
        let (key, source) = secrets::load_key();
        let _ = (TRANSCRIBE_MODEL, POLISH_MODEL);
        let phrases = store.phrases().unwrap_or_default();
        let candidates = store.pending_candidates().unwrap_or_default();
        let corrections = store.corrections();
        let auto_learn_mode = AutoLearnMode::from_str(&store.auto_learn_mode());
        let learn_from_edits = store.learn_from_edits();
        let last_mined_at = store.last_mined_at();
        let last_mine_error = store.last_mine_error();
        let history = store.history().unwrap_or_default();
        let audio_ids = Store::existing_audio_ids();
        let stats = store.stats();
        let show_dock = store.show_dock();
        let hotkey_code = store.hotkey_code();
        let trigger_mode = store.trigger_mode();
        let polish_strategy = store.polish_strategy();
        let custom_prompt = store.custom_prompt();
        let space_cjk = store.space_cjk();
        let sound_feedback = store.sound_feedback();
        let save_audio = store.save_audio();
        let mic_device = store.mic_device();
        let transcribe_model = store.transcribe_model();
        let polish_model = store.polish_model();
        let available_mics = crate::capture::list_input_devices();

        Self {
            phase: Phase::Idle,
            level: 0.0,
            wave: [0.0; crate::capture::WAVE_BARS],
            show_dock,
            hotkey_code,
            trigger_mode,
            polish_strategy,
            custom_prompt,
            space_cjk,
            sound_feedback,
            save_audio,
            mic_device,
            available_mics,
            transcribe_model,
            polish_model,
            key_status: settings::key_status(&key, &source),
            gateway_test_status: None,
            gateway_testing: false,
            phrases,
            candidates,
            corrections,
            auto_learn_mode,
            learn_from_edits,
            mining: false,
            mining_status: if last_mine_error.is_empty() {
                String::new()
            } else {
                format!("上次学习失败：{}", time::truncate_text(&last_mine_error, 80))
            },
            last_mined_at,
            audio_ids,
            history,
            stats,
            active_bundle: String::new(),
            playing_id: None,
            repolishing_id: None,
            studio_mode: PolishMode::Prose,
            studio_output: String::new(),
            studio_raw: String::new(),
            studio_busy: false,
            studio_dictating: false,
            mic_testing: false,
            key,
            store: Arc::new(Mutex::new(store)),
            recording: None,
            mic_test_stream: None,
            stages: None,
            generation: 0,
            note_id: 0,
            appearance: 0,
            exiting: false,
            hide_epoch: 0,
        }
    }
}
