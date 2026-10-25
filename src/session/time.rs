use std::time::Duration;

pub(super) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub(super) fn truncate_text(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

pub fn mmss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn format_duration_ms(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms as f32 / 1000.0;
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let m = (secs as u64) / 60;
        let s = (secs as u64) % 60;
        format!("{m}m {s:02}s")
    }
}

pub fn format_relative_time(ts: i64) -> String {
    if ts <= 0 {
        return crate::i18n::t("time.early").into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(ts);
    let diff = (now - ts).max(0);
    if diff < 45 {
        crate::i18n::t("time.just_now").into()
    } else if diff < 3600 {
        crate::i18n::t_args("time.minutes_ago", &[("n", &(diff / 60).to_string())])
    } else if diff < 86_400 {
        crate::i18n::t_args("time.hours_ago", &[("n", &(diff / 3600).to_string())])
    } else {
        crate::i18n::t_args("time.days_ago", &[("n", &(diff / 86_400).to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_mmss_correctly() {
        assert_eq!(mmss(Duration::from_secs(0)), "0:00");
        assert_eq!(mmss(Duration::from_secs(9)), "0:09");
        assert_eq!(mmss(Duration::from_secs(65)), "1:05");
        assert_eq!(mmss(Duration::from_secs(600)), "10:00");
    }

    #[test]
    fn formats_duration_ms_correctly() {
        assert_eq!(format_duration_ms(0), "");
        assert_eq!(format_duration_ms(-100), "");
        assert_eq!(format_duration_ms(1500), "1.5s");
        assert_eq!(format_duration_ms(65000), "1m 05s");
    }

    #[test]
    fn truncates_text_with_ellipsis() {
        assert_eq!(truncate_text("hello", 10), "hello");
        assert_eq!(truncate_text("hello world", 5), "hello…");
    }

    #[test]
    fn relative_time_handles_zero_and_future() {
        assert_eq!(format_relative_time(0), crate::i18n::t("time.early"));
    }
}
