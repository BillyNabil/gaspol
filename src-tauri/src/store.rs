//! Data model + penyimpanan JSON lokal (atomic write).

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Step {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub project: Option<String>,
    /// YYYY-MM-DD
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub est_minutes: Option<u32>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub done_at: Option<i64>,
    #[serde(default)]
    pub focus_secs: u64,
    #[serde(default)]
    pub snoozes: u32,
    #[serde(default)]
    pub last_nudge: Option<i64>,
    #[serde(default)]
    pub last_touched: Option<i64>,
    #[serde(default)]
    pub raw: String,
    #[serde(default)]
    pub ai: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Distractor {
    pub name: String,
    pub patterns: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// "none" | "claude" | "local"
    pub ai_provider: String,
    pub claude_model: String,
    pub local_base_url: String,
    pub local_model: String,
    pub custom_base_url: String,
    pub custom_model: String,
    /// "openai" | "anthropic"
    pub custom_format: String,
    pub hotkey: String,
    pub autostart: bool,
    pub report_hour: u32,
    pub grace_secs: u32,
    pub default_minutes: u32,
    /// "passive" | "nudge" | "strict"
    pub default_mode: String,
    pub distractors: Vec<Distractor>,
    pub first_run_done: bool,
    pub last_report_date: Option<String>,
    /// Jarak HUD dari tepi atas layar (px), biar tidak menimpa widget lain.
    pub hud_top: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_provider: "none".into(),
            claude_model: "claude-haiku-5-5".into(),
            local_base_url: "http://localhost:1234/v1".into(),
            local_model: String::new(),
            custom_base_url: String::new(),
            custom_model: String::new(),
            custom_format: "openai".into(),
            hotkey: "Ctrl+Space".into(),
            autostart: true,
            report_hour: 22,
            grace_secs: 4,
            default_minutes: 50,
            default_mode: "nudge".into(),
            distractors: default_distractors(),
            first_run_done: false,
            last_report_date: None,
            hud_top: 6,
        }
    }
}

/// Pola: `exe:nama` cocok ke nama proses; teks biasa dicocokkan ke judul tab browser.
pub fn default_distractors() -> Vec<Distractor> {
    let d = |name: &str, p: &[&str]| Distractor {
        name: name.into(),
        patterns: p.iter().map(|s| s.to_string()).collect(),
    };
    vec![
        d("YouTube", &["youtube"]),
        d("Netflix", &["netflix"]),
        d("Twitch", &["twitch"]),
        d("Discord", &["exe:discord", "discord"]),
        d("WhatsApp", &["exe:whatsapp", "exe:whatsapp.root", "whatsapp"]),
        d("Telegram", &["exe:telegram", "exe:unigram", "telegram"]),
        d("Steam", &["exe:steam", "exe:steamwebhelper"]),
        d("Roblox", &["exe:robloxplayerbeta"]),
        d("Instagram", &["instagram"]),
        d("TikTok", &["tiktok"]),
        d("X", &["/ x", "twitter"]),
        d("Reddit", &["reddit"]),
    ]
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AppStat {
    pub secs: u64,
    pub distracting: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SessionLog {
    pub task_id: Option<String>,
    pub task_title: String,
    pub start: i64,
    pub end: i64,
    pub planned_secs: u64,
    pub focus_secs: u64,
    pub distracted_secs: u64,
    pub distractions: u32,
    pub mode: String,
    pub completed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DayLog {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub apps: BTreeMap<String, AppStat>,
    #[serde(default)]
    pub sessions: Vec<SessionLog>,
    #[serde(default)]
    pub distractions: u32,
    #[serde(default)]
    pub snoozed: Vec<String>,
}

pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(dir.join("days"));
        Self { dir }
    }

    pub fn day_name(date: &str) -> String {
        format!("days/{date}.json")
    }

    pub fn load<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        let path = self.dir.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            return T::default();
        };
        match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => {
                // Jangan sampai file rusak tertimpa diam-diam: simpan cadangannya.
                let _ = fs::copy(&path, path.with_extension(format!("broken-{}.json", crate::now_ts())));
                T::default()
            }
        }
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("tmp");
        if let Ok(text) = serde_json::to_string_pretty(value) {
            if fs::write(&tmp, text).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }

    pub fn list_days(&self) -> Vec<String> {
        let mut days: Vec<String> = fs::read_dir(self.dir.join("days"))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.strip_suffix(".json")
                            .filter(|s| s.len() == 10)
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();
        days.sort();
        days
    }
}
