//! Deciding which language to speak.
//!
//! Order of precedence: the `jot lang` setting, then `JOT_LANG` and the usual
//! locale variables, then the OS locale, then English. Resolved once at
//! startup so every message downstream agrees.

use jot_core::i18n::{self, Lang};
use jot_core::Config;

/// The user's OS locale, e.g. `zh-CN`. `None` where we cannot ask.
#[cfg(windows)]
fn os_locale() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    // LOCALE_NAME_MAX_LENGTH is 85
    let mut buf = [0u16; 85];
    let written = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if written <= 1 {
        return None;
    }
    // The count includes the trailing NUL
    Some(String::from_utf16_lossy(&buf[..(written - 1) as usize]))
}

#[cfg(not(windows))]
fn os_locale() -> Option<String> {
    // Unix already exposes this through LANG / LC_*, which i18n reads directly
    None
}

/// Resolve the language and pin it for the rest of the process.
pub fn resolve(cfg: &Config) -> Lang {
    let lang = cfg
        .lang
        .as_deref()
        .and_then(Lang::parse)
        .or_else(i18n::from_env_opt)
        .or_else(|| os_locale().as_deref().and_then(Lang::parse))
        .unwrap_or(Lang::En);
    i18n::set(lang);
    lang
}

/// Where the current choice came from, for `jot lang` and `jot doctor`.
pub fn source(cfg: &Config) -> &'static str {
    if cfg.lang.as_deref().and_then(Lang::parse).is_some() {
        "jot lang"
    } else if i18n::from_env_opt().is_some() {
        "environment"
    } else if os_locale().as_deref().and_then(Lang::parse).is_some() {
        "OS locale"
    } else {
        "default"
    }
}
