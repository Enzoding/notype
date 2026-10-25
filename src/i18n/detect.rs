//! Reads the macOS preferred-language list. Query once at startup or when the
//! preference changes; never call this from a render path.
use objc2_foundation::NSLocale;

/// System languages in the user's priority order (e.g. `["zh-Hans-CN", "en-US"]`).
pub fn system_languages() -> Vec<String> {
    NSLocale::preferredLanguages()
        .iter()
        .map(|language| language.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_system_language_list_without_panicking() {
        // The list depends on the host, so only the call itself is asserted.
        let languages = super::system_languages();
        println!("system languages: {languages:?}");
    }
}
