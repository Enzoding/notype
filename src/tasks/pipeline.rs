//! Resumable processing: checkpoint each successful ASR chunk and preserve raw text on polish failure.
use super::{model::TaskPhase, Task};
use crate::{gateway, mining};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
pub struct Credentials {
    pub transcribe: String,
    pub polish: String,
}
pub fn process(
    task: &mut Task,
    keys: &Credentials,
    phrases: &[String],
    corrections: &[mining::CorrectionRule],
    cancelled: &Arc<AtomicBool>,
    stage: impl Fn(bool),
) -> Result<(), String> {
    let _scope = crate::providers::CancellationScope::enter(cancelled.clone());
    let result = run(task, keys, phrases, corrections, cancelled, stage);
    if let Err(ref error) = result {
        task.error = error.clone();
        task.phase = if cancelled.load(Ordering::Acquire) {
            TaskPhase::Cancelled
        } else {
            TaskPhase::Interrupted
        };
        task.save()?;
    }
    result
}
fn check(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("已取消，保留恢复草稿".into())
    } else {
        Ok(())
    }
}
fn run(
    task: &mut Task,
    keys: &Credentials,
    phrases: &[String],
    corrections: &[mining::CorrectionRule],
    cancelled: &AtomicBool,
    stage: impl Fn(bool),
) -> Result<(), String> {
    task.error.clear();
    check(cancelled)?;
    if task.transcript.is_empty() {
        let count = task.chunk_count()?;
        if count == 0 {
            return Err("没有可恢复的音频".into());
        }
        stage(false);
        task.phase = TaskPhase::Transcribing;
        task.save()?;
        for index in task.transcripts.len()..count {
            check(cancelled)?;
            if keys.transcribe.is_empty() {
                return Err("请配置此任务转写渠道的密钥".into());
            }
            let wav = task.chunk_wav(index)?;
            let p = &task.config.transcribe;
            let text = gateway::transcribe_with_model(
                p.kind,
                &p.base_url,
                &keys.transcribe,
                &wav,
                phrases,
                &p.transcribe_model,
            )?;
            check(cancelled)?;
            task.transcripts.push(text);
            task.save()?;
        }
        task.transcript = task.transcripts.join("\n");
        task.phase = TaskPhase::Transcribed;
        task.save()?;
    }
    check(cancelled)?;
    if task.corrected.is_empty() {
        task.corrected = mining::apply_corrections(&task.transcript, corrections);
        task.save()?;
    }
    if let Some(options) = &task.config.options {
        if task.polished.is_empty() {
            stage(true);
            task.phase = TaskPhase::Polishing;
            task.save()?;
            let corrected = task.corrected.clone();
            let p = &task.config.polish;
            let segments = crate::polish::segments(&corrected);
            for segment in segments.iter().skip(task.polish_parts.len()) {
                check(cancelled)?;
                let result = if keys.polish.is_empty() {
                    Err("请配置此任务润色渠道的密钥".into())
                } else {
                    gateway::polish_with_options(
                        &p.base_url,
                        &keys.polish,
                        segment,
                        phrases,
                        options,
                        &p.polish_model,
                    )
                };
                check(cancelled)?;
                match result {
                    Ok(text) => {
                        task.polish_parts.push(text);
                        task.save()?;
                    }
                    Err(error) => {
                        task.error = format!("润色失败：{error}");
                        task.phase = TaskPhase::PolishRetryPending;
                        task.save()?;
                        return Ok(());
                    }
                }
            }
            task.polished = task.polish_parts.join("\n\n");
        }
    }
    task.phase = TaskPhase::ReadyForDelivery;
    task.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retrying_polish_preserves_transcript_without_audio_or_transcription_key() {
        let mut task = crate::tasks::fixture();
        task.transcript = "synthetic transcript".into();
        task.config.options = Some(crate::polish::PolishOptions {
            mode: crate::polish::PolishMode::Prose,
            custom_prompt: String::new(),
            space_cjk: false,
        });
        process(
            &mut task,
            &Credentials {
                transcribe: String::new(),
                polish: String::new(),
            },
            &[],
            &[],
            &Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap();
        assert_eq!(task.transcript, "synthetic transcript");
        assert_eq!(task.phase, TaskPhase::PolishRetryPending);
        assert!(!task.error.is_empty());
        task.remove().unwrap();
    }
    #[test]
    fn cancelled_task_never_reaches_provider_or_delivery() {
        let mut task = crate::tasks::fixture();
        let keys = Credentials {
            transcribe: String::new(),
            polish: String::new(),
        };
        assert!(process(
            &mut task,
            &keys,
            &[],
            &[],
            &Arc::new(AtomicBool::new(true)),
            |_| panic!("cancelled task entered a stage")
        )
        .is_err());
        assert_eq!(task.phase, TaskPhase::Cancelled);
        task.remove().unwrap();
    }
    #[test]
    fn partial_polish_checkpoints_resume_without_repeating_completed_segments() {
        let mut task = crate::tasks::fixture();
        task.transcript = "合成测试文字。".repeat(250);
        task.corrected = task.transcript.clone();
        task.config.options = Some(crate::polish::PolishOptions {
            mode: crate::polish::PolishMode::Prose,
            custom_prompt: String::new(),
            space_cjk: false,
        });
        let count = crate::polish::segments(&task.corrected).len();
        assert!(count > 1);
        task.polish_parts = vec!["saved segment".into()];
        let keys = Credentials {
            transcribe: String::new(),
            polish: String::new(),
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        process(&mut task, &keys, &[], &[], &cancelled, |_| {}).unwrap();
        assert_eq!(task.polish_parts, vec!["saved segment"]);
        assert_eq!(task.phase, TaskPhase::PolishRetryPending);
        task.polish_parts
            .resize(count, "other saved segment".into());
        process(&mut task, &keys, &[], &[], &cancelled, |_| {}).unwrap();
        assert_eq!(task.phase, TaskPhase::ReadyForDelivery);
        assert!(task.polished.starts_with("saved segment\n\n"));
        task.remove().unwrap();
    }
    #[test]
    fn raw_transcript_can_resume_without_audio() {
        let mut task = crate::tasks::fixture();
        task.transcript = "raw synthetic text".into();
        process(
            &mut task,
            &Credentials {
                transcribe: String::new(),
                polish: String::new(),
            },
            &[],
            &[],
            &Arc::new(AtomicBool::new(false)),
            |_| panic!("finished transcription was repeated"),
        )
        .unwrap();
        assert_eq!(task.phase, TaskPhase::ReadyForDelivery);
        task.remove().unwrap();
    }
}
