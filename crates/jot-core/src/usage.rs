//! Frequency and recency (frecency).
//!
//! The heart of a retrieval problem is having your usual few at the top. The
//! more and the more recently an entry is used, the higher it ranks, so the
//!
//! tool improves with use. Stored in `~/.jot/usage.toml`; losing that file
//! only resets the ordering.

use crate::config::Paths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stat {
    pub count: u32,
    /// UNIX seconds
    pub last: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Usage(pub BTreeMap<String, Stat>);

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Usage {
    pub fn load(paths: &Paths) -> Usage {
        std::fs::read_to_string(paths.usage_file())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        paths.ensure()?;
        std::fs::write(paths.usage_file(), toml::to_string_pretty(&self.0)?)?;
        Ok(())
    }

    pub fn record(&mut self, id: &str) {
        let e = self.0.entry(id.to_string()).or_default();
        e.count = e.count.saturating_add(1);
        e.last = now_secs();
    }

    /// Ranking weight. Zero for anything unused, which keeps the original file order.
    ///
    /// Deliberately kept below the fuzzy-match scale: frequent entries float up,
    /// but never override an explicit search term - what you search for is what you get.
    pub fn boost(&self, id: &str, now: u64) -> i64 {
        let Some(s) = self.0.get(id) else { return 0 };
        let age = now.saturating_sub(s.last);
        const HOUR: u64 = 3_600;
        const DAY: u64 = 86_400;
        let recency = match age {
            a if a < HOUR => 8,
            a if a < DAY => 4,
            a if a < 7 * DAY => 2,
            a if a < 30 * DAY => 1,
            _ => 0,
        };
        // Sub-linear in count, so an entry used 500 times cannot squat the top
        let volume = (s.count as f64).sqrt().round() as i64;
        (volume + 1) * recency
    }

    /// The N most-used entries.
    pub fn top(&self, n: usize) -> Vec<(&str, &Stat)> {
        let mut v: Vec<(&str, &Stat)> = self.0.iter().map(|(k, s)| (k.as_str(), s)).collect();
        v.sort_by_key(|(_, s)| std::cmp::Reverse(s.count));
        v.truncate(n);
        v
    }

    pub fn total_uses(&self) -> u32 {
        self.0.values().map(|s| s.count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_entries_get_no_boost() {
        let u = Usage::default();
        assert_eq!(u.boost("git/never used", now_secs()), 0);
    }

    #[test]
    fn more_uses_ranks_higher() {
        let mut u = Usage::default();
        u.record("a");
        for _ in 0..9 {
            u.record("b");
        }
        let now = now_secs();
        assert!(
            u.boost("b", now) > u.boost("a", now),
            "the more-used entry did not rank higher"
        );
    }

    #[test]
    fn recent_beats_stale_at_equal_count() {
        let now = now_secs();
        let mut u = Usage::default();
        u.0.insert(
            "fresh".into(),
            Stat {
                count: 5,
                last: now,
            },
        );
        u.0.insert(
            "stale".into(),
            Stat {
                count: 5,
                last: now - 60 * 86_400,
            },
        );
        assert!(
            u.boost("fresh", now) > u.boost("stale", now),
            "the recently used entry did not rank higher"
        );
    }

    #[test]
    fn very_old_entries_decay_to_zero() {
        let now = now_secs();
        let mut u = Usage::default();
        u.0.insert(
            "ancient".into(),
            Stat {
                count: 100,
                last: now - 365 * 86_400,
            },
        );
        assert_eq!(
            u.boost("ancient", now),
            0,
            "an entry untouched for a year is still weighted"
        );
    }

    /// The weight must not override fuzzy matching - what you search for is what you get.
    #[test]
    fn boost_stays_below_fuzzy_scale() {
        let now = now_secs();
        let mut u = Usage::default();
        u.0.insert(
            "hot".into(),
            Stat {
                count: 10_000,
                last: now,
            },
        );
        assert!(
            u.boost("hot", now) < 1_000,
            "weight {} is too large and would swamp the search term",
            u.boost("hot", now)
        );
    }

    #[test]
    fn record_accumulates() {
        let mut u = Usage::default();
        u.record("x");
        u.record("x");
        assert_eq!(u.0["x"].count, 2);
        assert_eq!(u.total_uses(), 2);
    }

    #[test]
    fn top_is_sorted_by_count() {
        let mut u = Usage::default();
        u.record("low");
        for _ in 0..5 {
            u.record("high");
        }
        assert_eq!(u.top(1)[0].0, "high");
    }
}
