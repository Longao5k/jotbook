//! The data directory, config.toml and profiles.toml.
//!
//! All local plain text: notebooks can go into git, while profiles hold the
//! constants of your own environment and are deliberately kept out of it.

use crate::t;
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
        // JOT_HOME wins, which makes tests and portable installs easy
        if let Ok(p) = std::env::var("JOT_HOME") {
            if !p.trim().is_empty() {
                return Ok(Paths {
                    root: PathBuf::from(p),
                });
            }
        }
        let home = dirs::home_dir().context(t!(
            "找不到用户主目录",
            "cannot find the user's home directory"
        ))?;
        Ok(Paths {
            root: home.join(".jot"),
        })
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
    pub fn sources_dir(&self) -> PathBuf {
        self.notebooks().join("sources")
    }
    pub fn usage_file(&self) -> PathBuf {
        self.root.join("usage.toml")
    }

    pub fn notebook_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![self.builtin_dir(), self.local_dir()];
        // Loose .md files directly under notebooks/ count too
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
    /// The active profile name
    #[serde(default)]
    pub profile: Option<String>,
    /// Which version of the built-in notebooks is on disk
    #[serde(default)]
    pub builtin_version: Option<String>,
    /// How often the "install the shell integration" hint has been shown.
    #[serde(default)]
    pub hints_shown: u32,
    /// Explicitly trusted external sources. Only these get to run from: shell.
    #[serde(default)]
    pub trusted_sources: Vec<String>,
    /// Interface language chosen by the user: "en" / "zh". None follows the environment.
    #[serde(default)]
    pub lang: Option<String>,
    /// Which language's notebooks are currently in builtin/
    #[serde(default)]
    pub builtin_lang: Option<String>,
}

/// How many times a hint is worth repeating.
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
        self.0
            .get(profile)
            .and_then(|m| m.get(key))
            .map(|s| s.as_str())
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
