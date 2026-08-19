//! 数据目录、config.toml、profiles.toml。
//!
//! 全部是本地纯文本：notebooks 可以扔进 git，profiles 存的是你自己环境里的
//! 常量（服务名、主机、库名），刻意不进 git。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Paths> {
        // JOT_HOME 优先，方便测试和便携部署
        if let Ok(p) = std::env::var("JOT_HOME") {
            if !p.trim().is_empty() {
                return Ok(Paths { root: PathBuf::from(p) });
            }
        }
        let home = dirs::home_dir().context("找不到用户主目录")?;
        Ok(Paths { root: home.join(".jot") })
    }

    pub fn notebooks(&self) -> PathBuf {
        self.root.join("notebooks")
    }
    pub fn builtin_dir(&self) -> PathBuf {
        self.notebooks().join("builtin")
    }
    pub fn local_dir(&self) -> PathBuf {
        self.notebooks().join("local")
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    pub fn profiles_file(&self) -> PathBuf {
        self.root.join("profiles.toml")
    }

    pub fn notebook_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![self.builtin_dir(), self.local_dir()];
        // notebooks/ 根目录下的散装 md 也收
        v.push(self.notebooks());
        v
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(self.builtin_dir())?;
        std::fs::create_dir_all(self.local_dir())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// 当前激活的 Profile 名
    #[serde(default)]
    pub profile: Option<String>,
    /// 内置笔记本已落地的版本，用于升级时判断
    #[serde(default)]
    pub builtin_version: Option<String>,
    /// 「装 shell 集成」的提示已经显示过几次。显示够了就不再唠叨。
    #[serde(default)]
    pub hints_shown: u32,
}

/// 同一条提示最多说这么多次。
pub const HINT_LIMIT: u32 = 3;

impl Config {
    pub fn load(paths: &Paths) -> Config {
        std::fs::read_to_string(paths.config_file())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        paths.ensure()?;
        std::fs::write(paths.config_file(), toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn profile_name(&self) -> &str {
        self.profile.as_deref().unwrap_or("default")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Profiles(pub BTreeMap<String, BTreeMap<String, String>>);

impl Profiles {
    pub fn load(paths: &Paths) -> Profiles {
        std::fs::read_to_string(paths.profiles_file())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        paths.ensure()?;
        std::fs::write(paths.profiles_file(), toml::to_string_pretty(&self.0)?)?;
        Ok(())
    }

    pub fn get(&self, profile: &str, key: &str) -> Option<&str> {
        self.0.get(profile).and_then(|m| m.get(key)).map(|s| s.as_str())
    }

    pub fn set(&mut self, profile: &str, key: &str, value: &str) {
        self.0
            .entry(profile.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
    }

    pub fn names(&self) -> Vec<&str> {
        self.0.keys().map(|s| s.as_str()).collect()
    }

    pub fn entries(&self, profile: &str) -> Vec<(&str, &str)> {
        self.0
            .get(profile)
            .map(|m| m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect())
            .unwrap_or_default()
    }
}
