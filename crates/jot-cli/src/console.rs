//! Windows 控制台代码页。
//!
//! 内置笔记本的标题和说明全是中文。Windows Terminal 默认就是 UTF-8，但老的
//! conhost 上代码页常常是 936(GBK)，那样界面会整片乱码。这里在启动时切到
//! UTF-8，退出时还原 —— 不还原的话会影响用户在同一个窗口里跑的其它程序。

/// 进入 UTF-8 控制台；Drop 时还原原来的代码页。
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
