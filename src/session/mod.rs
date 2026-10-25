use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use gpui_kit::Context;

use crate::capture::ActiveRecording;
use crate::gateway::{POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::mining::{AutoLearnMode, CorrectionRule};
use crate::polish::PolishMode;
use crate::secrets;
use crate::store::{HistoryItem, Phrase, PhraseCandidate, StatsSummary, Store, TriggerMode};

mod history;
mod lifecycle;
mod mining;
mod providers;
mod recovery;
mod settings;
mod studio;
mod time;
mod types;

pub use time::{format_duration_ms, format_relative_time, mmss};
pub use types::{Notice, Phase, Stage};

pub use providers::ConnectionState;

pub struct Session {
    pub connections: ConnectionState,
    pub data_dir_display: String,
    pub recovery_tasks: Vec<crate::tasks::Summary>,
    pub recovery_status: String,
    pub(super) active_task: Option<String>,
    pub(super) cancelled: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub phase: Phase,
    pub level: f32,
    pub wave: [f32; crate::capture::WAVE_BARS],
    pub working_progress: f32,
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
    pub mic_refreshing: bool,
    pub accessibility_trusted: bool,
    pub microphone_access: crate::permissions::MicrophoneAccess,
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
    #[allow(dead_code)]
    pub studio_mode: PolishMode,
    #[allow(dead_code)]
    pub studio_output: String,
    #[allow(dead_code)]
    pub studio_raw: String,
    #[allow(dead_code)]
    pub studio_busy: bool,
    pub studio_dictating: bool,
    pub mic_testing: bool,
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
        let kinds = crate::providers::ProviderKind::ALL;
        let profiles = kinds.map(|kind| crate::providers::Profile::load(&store, kind));
        let kind = crate::providers::active_provider(&store);
        let transcribe_model = profiles[kind.index()].transcribe_model.clone();
        let polish_model = profiles[kind.index()].polish_model.clone();
        let mut provider_keys = [key.clone(), String::new()];
        let mut credential_status = settings::key_status(&key, &source);
        let openrouter_status = match secrets::load_provider(kinds[1]) {
            Ok(value) => {
                let status = settings::key_status(&value, &secrets::KeySource::Keychain);
                provider_keys[1] = value;
                status
            }
            Err(err) => err,
        };
        if kind == crate::providers::ProviderKind::OpenRouter {
            credential_status = openrouter_status;
        }
        let model_catalogs = kinds.map(|kind| {
            crate::providers::ModelCatalog::from_entries(
                store
                    .setting(&format!("models.{}", kind.id()))
                    .and_then(|v| serde_json::from_str(&v).ok())
                    .unwrap_or_default(),
            )
        });
        let available_mics = Vec::new();
        let selected_key = provider_keys[kind.index()].clone();

        Self {
            connections: ConnectionState {
                revision: 0,
                kind,
                checks: Default::default(),
                profiles,
                provider_keys,
                model_catalogs,
                models_loading: false,
                model_status: String::new(),
                transcribe_model,
                polish_model,
                key_status: credential_status,
                gateway_test_status: None,
                gateway_testing: false,
                key: selected_key,
            },
            data_dir_display: Store::data_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "无法读取数据目录".into()),
            recovery_tasks: Vec::new(),
            recovery_status: String::new(),
            active_task: None,
            cancelled: None,
            phase: Phase::Idle,
            level: 0.0,
            wave: [0.0; crate::capture::WAVE_BARS],
            working_progress: 0.0,
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
            mic_refreshing: false,
            microphone_access: crate::permissions::microphone_status(),
            accessibility_trusted: crate::hotkey::is_accessibility_trusted(),
            phrases,
            candidates,
            corrections,
            auto_learn_mode,
            learn_from_edits,
            mining: false,
            mining_status: if last_mine_error.is_empty() {
                String::new()
            } else {
                format!(
                    "上次学习失败：{}",
                    time::truncate_text(&last_mine_error, 80)
                )
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
