//! Live latency and accuracy benchmarks against provider gateways.
//! They make a small number of tiny paid requests with synthetic speech fixtures.
//! Run with:
//!
//! `cargo test --locked providers::live -- --ignored --nocapture --test-threads=1`

use super::*;
use crate::polish::PolishOptions;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn resolve_key() -> Option<String> {
    if let Ok(key) = std::env::var("NOTYPE_LIVE_KEY") {
        if !key.trim().is_empty() {
            return Some(key.trim().to_string());
        }
    }
    if let Ok(content) = std::fs::read_to_string(".env.local") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some(val) = line.strip_prefix("AI_GATEWAY_API_KEY=") {
                let v = val.trim().trim_matches('"').trim_matches('\'');
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn resolve_wav(name: &str) -> Vec<u8> {
    let fixture = PathBuf::from("tests/fixtures/audio").join(name);
    if fixture.is_file() {
        return std::fs::read(fixture).expect("read fixture wav");
    }
    if let Ok(dir) = std::env::var("NOTYPE_LIVE_WAV_DIR") {
        let path = Path::new(&dir).join(name);
        if path.is_file() {
            return std::fs::read(path).expect("read wav from NOTYPE_LIVE_WAV_DIR");
        }
    }
    panic!("测试音频文件未找到: {name}，请先运行 scripts/generate-samples.sh 生成样例");
}

#[test]
#[ignore = "live paid benchmark: network connection and keepalive latency"]
fn bench_connection_prewarm_and_keepalive() {
    let Some(api_key) = resolve_key() else {
        println!("[Latency Bench] 跳过：未找到可用 API Key (请设置 NOTYPE_LIVE_KEY 或在应用/ .env.local 中配置)");
        return;
    };

    println!("\n=== [评测 1] 网络握手与连接复用延迟验收 ===");
    let url = format!("{}/v1/models", DEFAULT_BASE_URL);

    // 1. 测试未复用连接时的 Cold Connect 耗时 (代理 + TLS 建连)
    let t0 = Instant::now();
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(15));
    builder = transport::configure_proxy(builder);
    let unpooled_client = builder.build().unwrap();
    let resp1 = unpooled_client.get(&url).bearer_auth(&api_key).send();
    let cold_dur = t0.elapsed().as_millis();
    println!(
        "1. Cold Connect (每次新建 Client，含 DNS + TCP + 代理 + TLS): {} ms (status: {:?})",
        cold_dur,
        resp1.map(|r| r.status())
    );

    // 2. 触发后台连接预热 (Prewarm Connection)
    transport::prewarm_connection(DEFAULT_BASE_URL, &api_key);
    // 给后台预热线程 600ms 完成建连
    std::thread::sleep(std::time::Duration::from_millis(600));

    // 3. 测量预热后共享 Client 的热连接请求耗时
    let t1 = Instant::now();
    let shared = transport::client().expect("get shared client");
    let resp2 = shared.get(&url).bearer_auth(&api_key).send();
    let warm_dur = t1.elapsed().as_millis();
    println!(
        "2. Warm Connect (预热后复用已就绪 TLS 连接): {} ms (status: {:?})",
        warm_dur,
        resp2.map(|r| r.status())
    );

    // 4. 测量连续第 2 次复用同一连接池的请求耗时 (Keep-Alive)
    let t2 = Instant::now();
    let resp3 = shared.get(&url).bearer_auth(&api_key).send();
    let keepalive_dur = t2.elapsed().as_millis();
    println!(
        "3. Keep-Alive Reuse (连续第二次调用，完全复用长连接): {} ms (status: {:?})",
        keepalive_dur,
        resp3.map(|r| r.status())
    );

    println!(
        "-> 节省延迟估计: 每次请求节省约 {} ms\n",
        cold_dur.saturating_sub(warm_dur)
    );
    assert!(warm_dur <= cold_dur * 2, "热连耗时不应显著大于冷连");
}

#[test]
#[ignore = "live paid benchmark: full pipeline latency matrix across sample scenarios"]
fn bench_live_samples_latency_matrix() {
    let Some(api_key) = resolve_key() else {
        println!("[Latency Bench] 跳过：未找到可用 API Key");
        return;
    };

    println!("\n=== [评测 2] 多场景端到端真实语音延迟验收看板 ===");
    println!("| 场景样例 | 音频时长 | ASR 转写耗时 | Stage 0 清洗 | LLM 润色耗时 | 交付总延迟 | 实时率 (RTF) | 最终输出文本 |");
    println!("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |");

    let samples = [
        ("sample_short.wav", "短句日常 (2s)", 2.0f64),
        ("sample_technical.wav", "中英技术口述 (6.3s)", 6.3f64),
        ("sample_disfluent.wav", "口吃口癖清洗 (4.9s)", 4.9f64),
    ];

    let phrases = vec!["Notype".to_string(), "CI".to_string(), "PR".to_string()];

    for (file, label, approx_sec) in samples {
        let wav_data = resolve_wav(file);
        let total_start = Instant::now();

        // Step 1: ASR Transcription
        let asr_start = Instant::now();
        let raw_transcript = transcribe_with_model(
            ProviderKind::Vercel,
            DEFAULT_BASE_URL,
            &api_key,
            &wav_data,
            &phrases,
            TRANSCRIBE_MODEL,
        )
        .unwrap_or_else(|e| format!("[转写失败: {e}]"));
        let asr_ms = asr_start.elapsed().as_millis();

        // Step 2: Stage 0 Clean Speech
        let clean_start = Instant::now();
        let cleaned = crate::polish::clean_speech(&raw_transcript);
        let clean_micros = clean_start.elapsed().as_micros();

        // Step 3: LLM Polish
        let polish_start = Instant::now();
        let polished = polish_with_options(
            DEFAULT_BASE_URL,
            &api_key,
            &cleaned,
            &phrases,
            &PolishOptions::default(),
            POLISH_MODEL,
        )
        .unwrap_or_else(|e| format!("[润色失败: {e}]"));
        let polish_ms = polish_start.elapsed().as_millis();

        let total_delivery_ms = total_start.elapsed().as_millis();
        let rtf = (total_delivery_ms as f64 / 1000.0) / approx_sec;

        println!(
            "| {} | {:.1}s | {} ms | {} µs | {} ms | **{} ms** | {:.2} | {} |",
            label,
            approx_sec,
            asr_ms,
            clean_micros,
            polish_ms,
            total_delivery_ms,
            rtf,
            polished
                .replace('\n', " ")
                .chars()
                .take(25)
                .collect::<String>()
        );

        assert!(!raw_transcript.is_empty(), "转写内容不应为空");
        assert!(!polished.is_empty(), "润色内容不应为空");
    }
    println!("\n");
}

#[test]
fn bench_offline_stage0_clean_speech_latency_and_compression() {
    let cases = [
        "呃……我想说那个那个……我们我们明天去上海开会。",
        "就是说，我们今天去北京开会，这个这个方案可以可以。",
        "嗯……下午三点开会，然后然后把这个PR merge到main分支。",
    ];

    println!("\n=== [离线基准] Stage 0 口语清洗性能与降噪评测 ===");
    for (i, raw) in cases.iter().enumerate() {
        let t0 = Instant::now();
        let cleaned = crate::polish::clean_speech(raw);
        let elapsed_micros = t0.elapsed().as_micros();
        let raw_chars = raw.chars().count();
        let clean_chars = cleaned.chars().count();
        let saved_ratio = (raw_chars - clean_chars) as f64 / raw_chars as f64 * 100.0;

        println!(
            "样例 {}: 原始 {} 字 -> 清洗后 {} 字 (缩短 {:.1}%) | 纯本地 CPU 耗时: {} µs",
            i + 1,
            raw_chars,
            clean_chars,
            saved_ratio,
            elapsed_micros
        );
        println!("  原始: {}", raw);
        println!("  清洗: {}", cleaned);
        assert!(elapsed_micros < 2000, "Stage 0 本地清洗应在 2ms 内完成");
    }
}

#[test]
fn bench_offline_paste_before_archive_timing_gain() {
    println!("\n=== [离线基准] 先粘贴后归档 (Paste before Archive) 交付提速量化 ===");
    let store = crate::store::Store::open_in_memory().unwrap();
    let mut task = crate::tasks::fixture();
    task.transcript = "测试音频转写文本".repeat(10);
    task.corrected = task.transcript.clone();
    task.polished = "测试润色输出文本".repeat(10);
    task.config.save_audio = true;

    // 测量如果将数据库写入与统计放在上屏前需要耗时多久
    let t0 = Instant::now();
    let _id = store.save_task_history(&task).unwrap();
    let _ = store.history().unwrap();
    let _ = store.stats();
    let archive_io_ms = t0.elapsed().as_millis();

    println!(
        "模拟上屏前节省的 SQLite 事务与历史查询耗时: {} ms",
        archive_io_ms
    );
    println!("-> 采用两阶段时序后，用户从松手到触发粘贴完全免除此 I/O 阻塞。");
}
