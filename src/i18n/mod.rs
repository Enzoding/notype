//! Interface language: preference resolution, system detection, and message
//! lookup. Independent of GPUI, HTTP, and SQLite; callers persist the
//! preference and decide when to refresh their views.
mod catalog;
mod detect;
mod lang;

#[allow(unused_imports)]
// `t_args` is first used by parameterized copy in the next migration step
pub use catalog::{set_language, t, t_args};
pub use lang::{resolve, Language, Preference};

/// Resolve `preference` against the system language list and make it current.
/// Queries the OS, so call it at startup or when the preference changes.
pub fn apply(preference: Preference) -> Language {
    let language = resolve(preference, &detect::system_languages());
    set_language(language);
    language
}
