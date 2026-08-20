//! Bilingual user-facing text.
//!
//! Everything the user reads exists in both English and Chinese: CLI output,
//! built-in notebooks, and the README. Source comments stay English-only —
//! they are for contributors, and keeping two copies in sync is not worth it.
//!
//! Resolution order: an explicit `jot lang` setting, then `JOT_LANG`, then the
//! usual locale variables, then the OS locale (supplied by the front end).
//! English is the fallback, because that is the right default for a project
//! that anyone might clone.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Zh,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Zh => "zh",
        }
    }

    /// Accepts anything locale-shaped: `zh`, `zh-CN`, `zh_CN.UTF-8`, `en_AU`.
    pub fn parse(s: &str) -> Option<Lang> {
        let s = s.trim().to_ascii_lowercase();
        if s.starts_with("zh") {
            Some(Lang::Zh)
        } else if s.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }
}

/// 0 = not resolved yet, 1 = English, 2 = Chinese.
///
/// Deliberately not a OnceLock: `jot lang zh` has to switch language *and*
/// re-seed the built-in notebooks within the same process, which a
/// write-once cell makes impossible.
static LANG: AtomicU8 = AtomicU8::new(0);

/// Set the language. Always takes effect, including over an earlier value.
pub fn set(lang: Lang) {
    let code = match lang {
        Lang::En => 1,
        Lang::Zh => 2,
    };
    LANG.store(code, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::Zh,
        _ => {
            let resolved = from_env();
            set(resolved);
            resolved
        }
    }
}

/// Language implied by the environment alone. `None` means "nothing said".
pub fn from_env_opt() -> Option<Lang> {
    for key in ["JOT_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if let Some(lang) = Lang::parse(&value) {
                return Some(lang);
            }
        }
    }
    None
}

fn from_env() -> Lang {
    from_env_opt().unwrap_or(Lang::En)
}

/// Pick between a Chinese and an English string, with `format!` arguments.
///
/// Both variants sit side by side at the call site, which is the only way
/// they stay in sync in practice.
///
/// ```ignore
/// eprintln!("{}", t!("已存入 {}", "saved to {}", path.display()));
/// ```
#[macro_export]
macro_rules! t {
    // format! even with no extra arguments, so inline captures like `{shell}`
    // still work. Borrowing instead would silently leave them as literal text.
    ($zh:expr, $en:expr $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Zh => ::std::borrow::Cow::<str>::Owned(format!($zh)),
            $crate::i18n::Lang::En => ::std::borrow::Cow::<str>::Owned(format!($en)),
        }
    };
    ($zh:expr, $en:expr, $($arg:tt)+) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Zh => ::std::borrow::Cow::<str>::Owned(format!($zh, $($arg)+)),
            $crate::i18n::Lang::En => ::std::borrow::Cow::<str>::Owned(format!($en, $($arg)+)),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_strings_are_recognised() {
        for s in ["zh", "zh-CN", "zh_CN.UTF-8", "ZH-Hans", "zh-TW"] {
            assert_eq!(Lang::parse(s), Some(Lang::Zh), "{s}");
        }
        for s in ["en", "en-US", "en_AU.UTF-8", "EN"] {
            assert_eq!(Lang::parse(s), Some(Lang::En), "{s}");
        }
        for s in ["", "C", "POSIX", "ja-JP", "de"] {
            assert_eq!(Lang::parse(s), None, "{s}");
        }
    }

    #[test]
    fn codes_round_trip() {
        for lang in [Lang::En, Lang::Zh] {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
        }
    }
}
