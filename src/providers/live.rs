//! Manual smoke tests against the real Vercel AI Gateway. They are ignored by
//! default, make a handful of tiny paid requests with synthetic speech, and never
//! print the key. Run (key from the environment, never from the command line):
//!
//! `NOTYPE_LIVE_KEY=… NOTYPE_LIVE_WAV_DIR=/dir/with/en.wav,zh.wav \
//!  cargo test --locked providers::live -- --ignored --nocapture --test-threads=1`
use super::*;
use crate::polish::PolishOptions;

fn key() -> String {
    std::env::var("NOTYPE_LIVE_KEY").expect("set NOTYPE_LIVE_KEY")
}

fn wav(name: &str) -> Vec<u8> {
    let dir = std::env::var("NOTYPE_LIVE_WAV_DIR").expect("set NOTYPE_LIVE_WAV_DIR");
    std::fs::read(std::path::Path::new(&dir).join(name)).expect("read synthetic wav")
}

#[test]
#[ignore = "live paid request"]
fn live_chat_connection_and_model_catalog() {
    let ms = test_connection(DEFAULT_BASE_URL, &key(), POLISH_MODEL).expect("chat connection");
    println!("chat ok in {ms} ms");
    let store = crate::store::Store::open_in_memory().unwrap();
    let profile = Profile::load(&store, ProviderKind::Vercel);
    let models = fetch_models(&profile, &key()).expect("model catalog");
    println!("catalog: {} entries", models.len());
    assert!(!models.is_empty());
}

#[test]
#[ignore = "live paid request"]
fn live_transcribe_polish_and_glossary_round_trip() {
    for (file, phrases) in [
        ("en.wav", vec!["Notype".to_string()]),
        ("zh.wav", vec!["Notype".to_string()]),
    ] {
        let raw = transcribe_with_model(
            ProviderKind::Vercel,
            DEFAULT_BASE_URL,
            &key(),
            &wav(file),
            &phrases,
            TRANSCRIBE_MODEL,
        )
        .unwrap_or_else(|e| panic!("{file} transcription failed: {e}"));
        println!("{file} raw      : {raw}");
        let polished = polish_with_options(
            DEFAULT_BASE_URL,
            &key(),
            &raw,
            &phrases,
            &PolishOptions::default(),
            POLISH_MODEL,
        )
        .unwrap_or_else(|e| panic!("{file} polish failed: {e}"));
        println!("{file} polished : {polished}");
        assert!(!polished.trim().is_empty());
    }
    let glossary = extract_glossary(
        DEFAULT_BASE_URL,
        &key(),
        POLISH_MODEL,
        "Return JSON {\"terms\":[]} listing product names in: We shipped Notype today.",
    )
    .expect("glossary");
    println!("glossary: {glossary}");
}
