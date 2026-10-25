use super::{Phase, Session};
use gpui_kit::Context;
impl Session {
    pub fn refresh_recovery(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() || matches!(self.phase, Phase::Working(_)) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { crate::tasks::recover() })
                .await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(tasks) => s.recovery_tasks = tasks,
                    Err(err) => s.recovery_status = err,
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn retry_task(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.recording.is_some() || matches!(self.phase, Phase::Working(_)) {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.active_task = Some(id.into());
        self.launch_task(Err("恢复任务没有新录音".into()), Some(id.into()), true, cx);
    }
    pub fn delete_task(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.active_task.as_deref() == Some(id) {
            return;
        }
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::tasks::remove_task(&id) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if let Err(e) = result {
                    s.recovery_status = e;
                }
                s.refresh_recovery(cx);
            });
        })
        .detach();
    }
}
