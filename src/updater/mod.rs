//! In-app update checking, downloading, and installation for Notype.

pub mod client;
pub mod installer;
pub mod version;

use std::path::PathBuf;

pub use client::{check_for_updates, download_release, ReleaseInfo};
pub use installer::install_and_restart;
#[allow(unused_imports)]
pub use version::is_newer;

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        version: String,
    },
    Available(ReleaseInfo),
    Downloading {
        version: String,
        progress: f32,
        received: u64,
        total: u64,
    },
    Downloaded {
        version: String,
        dmg_path: PathBuf,
        release_info: ReleaseInfo,
    },
    Installing,
    Error(String),
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
