use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::version::is_newer;

const GITHUB_REPO: &str = "Enzoding/notype";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub download_url: String,
    pub asset_url: Option<String>,
    pub asset_name: String,
    pub asset_size: u64,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    url: String,
    size: u64,
}

fn build_client() -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::limited(10));
    builder = crate::providers::transport::configure_proxy(builder);
    builder
        .build()
        .map_err(|e| format!("无法创建网络客户端: {e}"))
}

pub fn check_for_updates() -> Result<Option<ReleaseInfo>, String> {
    let client = build_client()?;
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let mut req = client
        .get(&url)
        .header("User-Agent", format!("Notype/{CURRENT_VERSION}"))
        .header("Accept", "application/vnd.github.v3+json");

    if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
        let token = token.trim();
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
    }

    let resp = req
        .send()
        .map_err(|e| format!("请求 GitHub 失败，请检查网络或代理连接: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(
            "检查更新返回 404。若 GitHub 仓库为私有，请在设置中公开或配置 GITHUB_TOKEN。".into(),
        );
    }
    if !status.is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(format!("检查更新失败（状态码 {status}）: {text}"));
    }

    let release: GitHubRelease = resp
        .json()
        .map_err(|e| format!("解析 Release 数据失败: {e}"))?;

    let remote_version = release.tag_name.trim_start_matches('v').trim().to_string();
    if !is_newer(&remote_version, CURRENT_VERSION) {
        return Ok(None);
    }

    let dmg_asset = release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with(".dmg"))
        .ok_or_else(|| "最新 Release 未找到 .dmg 安装包".to_string())?;

    let name = release
        .name
        .unwrap_or_else(|| format!("Notype v{remote_version}"));

    Ok(Some(ReleaseInfo {
        version: remote_version,
        tag_name: release.tag_name,
        name,
        body: release.body.unwrap_or_default(),
        download_url: dmg_asset.browser_download_url,
        asset_url: Some(dmg_asset.url),
        asset_name: dmg_asset.name,
        asset_size: dmg_asset.size,
    }))
}

pub fn download_release<F>(
    info: &ReleaseInfo,
    target_dir: &Path,
    mut on_progress: F,
) -> Result<PathBuf, String>
where
    F: FnMut(f32, u64, u64),
{
    std::fs::create_dir_all(target_dir).map_err(|e| format!("创建更新目录失败: {e}"))?;

    let final_path = target_dir.join(&info.asset_name);
    let part_path = target_dir.join(format!("{}.part", info.asset_name));

    let client = build_client()?;
    let token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok();

    let req = if let (Some(token), Some(asset_url)) = (token.as_ref(), info.asset_url.as_ref()) {
        client
            .get(asset_url)
            .header("User-Agent", format!("Notype/{CURRENT_VERSION}"))
            .header("Authorization", format!("Bearer {}", token.trim()))
            .header("Accept", "application/octet-stream")
    } else {
        client
            .get(&info.download_url)
            .header("User-Agent", format!("Notype/{CURRENT_VERSION}"))
    };

    let mut resp = req.send().map_err(|e| format!("下载安装包失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载请求失败（状态码 {}）", resp.status()));
    }

    let total = resp.content_length().unwrap_or(info.asset_size);
    let mut file =
        std::fs::File::create(&part_path).map_err(|e| format!("无法创建临时文件: {e}"))?;

    let mut buffer = [0u8; 32768];
    let mut received: u64 = 0;

    loop {
        let n = resp
            .read(&mut buffer)
            .map_err(|e| format!("下载中断: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buffer[..n])
            .map_err(|e| format!("写入文件失败: {e}"))?;
        received += n as u64;
        let progress = if total > 0 {
            (received as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        on_progress(progress, received, total);
    }

    std::fs::rename(&part_path, &final_path).map_err(|e| format!("重命名更新文件失败: {e}"))?;

    Ok(final_path)
}
