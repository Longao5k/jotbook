//! Windows console code page.
//!
//! Windows Terminal is UTF-8 already, but legacy conhost often runs a
//! regional code page such as 936 (GBK), where non-ASCII output turns to
//! mojibake. Switch to UTF-8 on start and restore it on exit; not restoring
//! would affect other programs run in the same window afterwards.

/// Enter a UTF-8 console; the previous code page is restored on Drop.
pub struct Utf8Console {
    #[cfg(windows)]
    previous_output: u32,
    #[cfg(windows)]
    previous_input: u32,
}

#[cfg(windows)]
impl Utf8Console {
    pub fn enter() -> Utf8Console {
        use windows_sys::Win32::System::Console::{
            GetConsoleCP, GetConsoleOutputCP, SetConsoleCP, SetConsoleOutputCP,
        };
        const UTF8: u32 = 65001;
        unsafe {
            let previous_output = GetConsoleOutputCP();
            let previous_input = GetConsoleCP();
            if previous_output != UTF8 {
                SetConsoleOutputCP(UTF8);
            }
            if previous_input != UTF8 {
                SetConsoleCP(UTF8);
            }
            Utf8Console {
                previous_output,
                previous_input,
            }
        }
    }
}

#[cfg(windows)]
impl Drop for Utf8Console {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};
        const UTF8: u32 = 65001;
        unsafe {
            if self.previous_output != UTF8 && self.previous_output != 0 {
                SetConsoleOutputCP(self.previous_output);
            }
            if self.previous_input != UTF8 && self.previous_input != 0 {
                SetConsoleCP(self.previous_input);
            }
        }
    }
}

#[cfg(not(windows))]
impl Utf8Console {
    pub fn enter() -> Utf8Console {
        Utf8Console {}
    }
}
