use std::time::Duration;

use gpui_kit::Context;

use super::Session;
use crate::updater::{self, UpdateState};

enum DownloadEvent {
    Progress {
        progress: f32,
        received: u64,
        total: u64,
    },
    Done(Result<std::path::PathBuf, String>),
}

impl Session {
    pub fn check_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.update_state,
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing
        ) {
            return;
        }
        self.update_state = UpdateState::Checking;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { updater::check_for_updates() })
                .await;

            let _ = this.update(cx, |session, cx| {
                session.update_state = match res {
                    Ok(Some(info)) => UpdateState::Available(info),
                    Ok(None) => UpdateState::UpToDate {
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    Err(err) => UpdateState::Error(err),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub fn download_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(info) = &self.update_state else {
            return;
        };
        let info = info.clone();
        let target_dir = match crate::store::Store::data_dir() {
            Ok(d) => d.join("updates"),
            Err(e) => {
                self.update_state = UpdateState::Error(e.to_string());
                cx.notify();
                return;
            }
        };

        self.update_state = UpdateState::Downloading {
            version: info.version.clone(),
            progress: 0.0,
            received: 0,
            total: info.asset_size,
        };
        cx.notify();

        let (tx, rx) = std::sync::mpsc::channel();
        let info_clone = info.clone();
        let tx_progress = tx.clone();

        cx.background_executor()
            .spawn(async move {
                let res = updater::download_release(
                    &info_clone,
                    &target_dir,
                    move |progress, received, total| {
                        let _ = tx_progress.send(DownloadEvent::Progress {
                            progress,
                            received,
                            total,
                        });
                    },
                );
                let _ = tx.send(DownloadEvent::Done(res));
            })
            .detach();

        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let mut last_progress = None;
            let mut done = None;
            while let Ok(event) = rx.try_recv() {
                match event {
                    DownloadEvent::Progress {
                        progress,
                        received,
                        total,
                    } => {
                        last_progress = Some((progress, received, total));
                    }
                    DownloadEvent::Done(res) => {
                        done = Some(res);
                        break;
                    }
                }
            }
            if let Some((p, r, t)) = last_progress {
                let _ = this.update(cx, |s, cx| {
                    if let UpdateState::Downloading {
                        progress,
                        received,
                        total,
                        ..
                    } = &mut s.update_state
                    {
                        *progress = p;
                        *received = r;
                        *total = t;
                        cx.notify();
                    }
                });
            }
            if let Some(res) = done {
                let _ = this.update(cx, |s, cx| {
                    s.update_state = match res {
                        Ok(dmg_path) => UpdateState::Downloaded {
                            version: info.version.clone(),
                            dmg_path,
                            release_info: info,
                        },
                        Err(e) => UpdateState::Error(e),
                    };
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }

    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Downloaded { dmg_path, .. } = &self.update_state else {
            return;
        };
        let dmg_path = dmg_path.clone();
        self.update_state = UpdateState::Installing;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { updater::install_and_restart(&dmg_path) })
                .await;

            if let Err(err) = res {
                let _ = this.update(cx, |s, cx| {
                    s.update_state = UpdateState::Error(err);
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
