use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
thread_local! { static CANCELLED: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) }; }
pub(crate) struct CancellationScope(Option<Arc<AtomicBool>>);
impl CancellationScope {
    pub(crate) fn enter(cancelled: Arc<AtomicBool>) -> Self {
        Self(CANCELLED.with(|slot| slot.replace(Some(cancelled))))
    }
}
impl Drop for CancellationScope {
    fn drop(&mut self) {
        CANCELLED.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}
fn check_cancelled() -> Result<(), String> {
    if CANCELLED.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Acquire))
    }) {
        Err("已取消".into())
    } else {
        Ok(())
    }
}

struct ClientCache {
    proxy: Option<String>,
    checked_at: std::time::Instant,
    client: reqwest::blocking::Client,
}

static SHARED_CLIENT: std::sync::Mutex<Option<ClientCache>> = std::sync::Mutex::new(None);

pub fn prewarm_connection(base_url: &str, key: &str) {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() || !base.starts_with("http") {
        return;
    }
    let url = format!("{base}/v1/models");
    let key = key.to_string();
    std::thread::spawn(move || {
        if let Ok(c) = client() {
            let mut req = c.get(&url).timeout(std::time::Duration::from_secs(3));
            if !key.is_empty() {
                req = req.bearer_auth(&key);
            }
            let _ = req.send();
        }
    });
}

fn current_effective_proxy() -> Option<String> {
    for var in ["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"] {
        if let Ok(val) = std::env::var(var) {
            if !val.trim().is_empty() {
                return Some(val);
            }
        }
    }
    detect_macos_system_proxy()
}

pub(super) fn client() -> Result<reqwest::blocking::Client, String> {
    let mut guard = SHARED_CLIENT.lock().map_err(|_| "客户端锁异常")?;
    let now = std::time::Instant::now();
    if let Some(ref mut cache) = *guard {
        if now.duration_since(cache.checked_at) < std::time::Duration::from_secs(30) {
            return Ok(cache.client.clone());
        }
        let current_proxy = current_effective_proxy();
        if current_proxy == cache.proxy {
            cache.checked_at = now;
            return Ok(cache.client.clone());
        }
    }

    let proxy = current_effective_proxy();
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(90))
        .pool_idle_timeout(std::time::Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none());

    if let Some(ref proxy_url) = proxy {
        if let Ok(p) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(p);
        }
    }

    let built = builder
        .build()
        .map_err(|_| "无法创建网络客户端，请检查代理配置".to_string())?;

    *guard = Some(ClientCache {
        proxy,
        checked_at: now,
        client: built.clone(),
    });

    Ok(built)
}

pub(crate) fn configure_proxy(
    mut builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    // If environment variables already specify a proxy (e.g. in terminal), reqwest respects them.
    // However, macOS GUI applications launched from Finder / launchd do not inherit shell env vars.
    // In that case, automatically detect and apply the active macOS system proxy:
    if std::env::var("https_proxy").is_err()
        && std::env::var("HTTPS_PROXY").is_err()
        && std::env::var("all_proxy").is_err()
        && std::env::var("ALL_PROXY").is_err()
    {
        if let Some(proxy_url) = detect_macos_system_proxy() {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
    }
    builder
}

pub fn detect_macos_system_proxy() -> Option<String> {
    let output = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_scutil_proxy(&text)
}

pub fn parse_scutil_proxy(text: &str) -> Option<String> {
    let mut https_enabled = false;
    let mut https_host = String::new();
    let mut https_port = 0u16;

    let mut socks_enabled = false;
    let mut socks_host = String::new();
    let mut socks_port = 0u16;

    let mut http_enabled = false;
    let mut http_host = String::new();
    let mut http_port = 0u16;

    for line in text.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "HTTPSEnable" => https_enabled = val == "1",
                "HTTPSProxy" => https_host = val.to_string(),
                "HTTPSPort" => https_port = val.parse().unwrap_or(0),
                "HTTPEnable" => http_enabled = val == "1",
                "HTTPProxy" => http_host = val.to_string(),
                "HTTPPort" => http_port = val.parse().unwrap_or(0),
                "SOCKSEnable" => socks_enabled = val == "1",
                "SOCKSProxy" => socks_host = val.to_string(),
                "SOCKSPort" => socks_port = val.parse().unwrap_or(0),
                _ => {}
            }
        }
    }

    if https_enabled && !https_host.is_empty() && https_port > 0 {
        return Some(format!("http://{https_host}:{https_port}"));
    }
    if http_enabled && !http_host.is_empty() && http_port > 0 {
        return Some(format!("http://{http_host}:{http_port}"));
    }
    if socks_enabled && !socks_host.is_empty() && socks_port > 0 {
        return Some(format!("socks5h://{socks_host}:{socks_port}"));
    }
    None
}

pub(super) fn send_chat(
    client: &reqwest::blocking::Client,
    url: &str,
    key: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, String> {
    send_with_retry(|| client.post(url).bearer_auth(key).json(&body).send())
}
pub(super) fn send_with_retry(
    mut request: impl FnMut() -> Result<reqwest::blocking::Response, reqwest::Error>,
) -> Result<reqwest::blocking::Response, String> {
    for attempt in 0..3 {
        check_cancelled()?;
        let response = request().map_err(|err| {
            if err.is_timeout() {
                "请求超时，结果不确定；可手动重试，但渠道可能重复收费".to_string()
            } else {
                "网络请求失败，请检查网络或代理后重试".to_string()
            }
        })?;
        check_cancelled()?;
        if !retry_status(response.status().as_u16()) || attempt == 2 {
            return Ok(response);
        }
        let delay = if let Some(header) = response.headers().get(reqwest::header::RETRY_AFTER) {
            match header.to_str().ok().and_then(|s| s.parse::<u64>().ok()) {
                Some(seconds) => seconds,
                None => return Ok(response),
            }
        } else {
            1 << attempt
        };
        // Long server-directed waits belong to manual retry rather than blocking a worker indefinitely.
        if delay > 15 {
            return Ok(response);
        }
        drop(response);
        let jitter = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis() as u64
            % 250;
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(delay * 1000 + jitter);
        while std::time::Instant::now() < deadline {
            check_cancelled()?;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    unreachable!()
}
fn retry_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}
pub(super) fn error_message(status: reqwest::StatusCode, _body: &str) -> String {
    let hint = match status.as_u16() {
        401 | 403 => "请检查当前渠道的密钥或权限",
        413 => "音频超出渠道限制，请使用较短分片",
        429 => "渠道限流或额度不足，请稍后重试",
        _ => "请求失败，请检查模型与渠道配置",
    };
    // Upstream bodies may echo dictated text, prompts or credentials. Never surface them in diagnostics.
    format!("渠道返回 {status}：{hint}")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mock_response(statuses: Vec<u16>) -> (String, std::thread::JoinHandle<usize>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut received = 0;
            while received < statuses.len() && std::time::Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
                            .unwrap();
                        let mut request = [0; 4096];
                        let _ = stream.read(&mut request);
                        let status = statuses[received];
                        write!(stream,"HTTP/1.1 {status} Test\r\nRetry-After: 0\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
                        received += 1;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
            received
        });
        (format!("http://{address}"), handle)
    }
    #[test]
    fn retries_rate_limit_response_but_not_invalid_key() {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let (url, server) = mock_response(vec![429, 200]);
        assert_eq!(
            send_with_retry(|| client.get(&url).send())
                .unwrap()
                .status(),
            200
        );
        assert_eq!(server.join().unwrap(), 2);
        let (url, server) = mock_response(vec![401]);
        assert_eq!(
            send_with_retry(|| client.get(&url).send())
                .unwrap()
                .status(),
            401
        );
        assert_eq!(server.join().unwrap(), 1);
    }
    #[test]
    fn cancellation_after_response_prevents_any_retry() {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .unwrap();
        let (url, server) = mock_response(vec![503]);
        let cancelled = Arc::new(AtomicBool::new(false));
        let _scope = CancellationScope::enter(cancelled.clone());
        assert!(send_with_retry(|| {
            let response = client.get(&url).send();
            cancelled.store(true, Ordering::Release);
            response
        })
        .is_err());
        assert_eq!(server.join().unwrap(), 1);
    }
    #[test]
    fn cancelled_scope_blocks_request_and_restores_previous_context() {
        let scope = CancellationScope::enter(Arc::new(AtomicBool::new(true)));
        assert!(send_with_retry(|| panic!("request should not be sent")).is_err());
        drop(scope);
        assert!(check_cancelled().is_ok());
    }
    #[test]
    fn retries_only_transient_responses() {
        assert!(retry_status(429));
        assert!(retry_status(503));
        assert!(!retry_status(401));
        assert!(!retry_status(413));
    }
    #[test]
    fn parses_system_proxy() {
        assert_eq!(
            parse_scutil_proxy("HTTPSEnable : 1\nHTTPSProxy : 127.0.0.1\nHTTPSPort : 7890"),
            Some("http://127.0.0.1:7890".into())
        );
    }

    #[test]
    fn shared_client_reuses_instance() {
        let c1 = client().unwrap();
        let c2 = client().unwrap();
        assert!(format!("{:?}", c1).contains("Client"));
        assert!(format!("{:?}", c2).contains("Client"));
    }

    #[test]
    fn prewarm_ignores_empty_or_invalid_endpoint() {
        prewarm_connection("", "");
        prewarm_connection("ftp://invalid", "");
    }
}
