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
    /// Waktu yang tercatat otomatis (tanpa sesi) karena jendela aktif cocok dengan tugas ini.
    #[serde(default)]
    pub auto_secs: u64,
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
    /// App/pola yang "sah" untuk tugas ini (`exe:blender`, `figma`, ...). Kosong = bebas.
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Distractor {
    pub name: String,
    pub patterns: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct WorkPreset {
    pub id: String,
    pub name: String,
    /// URL, path file/folder, atau nama app (`code`, `afterfx`).
    pub open: Vec<String>,
    pub close_distractors: bool,
    pub minutes: u32,
    pub mode: String,
    pub project: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Template {
    pub name: String,
    pub keywords: Vec<String>,
    pub steps: Vec<String>,
    pub est_minutes: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct GitRepo {
    pub path: String,
    pub project: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// "none" | "claude" | "local" | "custom"
    pub ai_provider: String,
    pub claude_model: String,
    pub local_base_url: String,
    pub local_model: String,
    pub custom_base_url: String,
    pub custom_model: String,
    /// "openai" | "anthropic"
    pub custom_format: String,
    pub hotkey: String,
    /// Tangkap teks yang sedang diblok.
    pub selection_hotkey: String,
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
    /// 0 = monitor utama, 1.. = monitor ke-n (urut kiri → kanan).
    pub hud_monitor: u32,

    // --- fokus pintar
    /// Judul yang mengandung kata ini tidak dihitung ngelantur (tutorial Blender di YouTube, dll).
    pub allow_keywords: Vec<String>,
    /// Buka pengganggu → ditanya "berapa menit?" dulu, bukan langsung ditegur.
    pub intent_mode: bool,
    /// Lagi flow saat waktu habis → sesi diperpanjang 15 menit diam-diam.
    pub flow_protect: bool,
    /// Pengingat istirahat setelah sekian menit aktif nonstop (0 = mati).
    pub break_every_min: u32,
    /// Blokir situs lewat file hosts selama sesi Galak (minta izin admin).
    pub hard_block: bool,
    pub block_sites: Vec<String>,
    /// "" = mati, "media" = tombol play/pause, atau URL/URI playlist.
    pub music: String,

    // --- mode kerja & template
    pub presets: Vec<WorkPreset>,
    pub templates: Vec<Template>,

    // --- render & export
    pub blender_path: String,
    pub aerender_path: String,
    /// "none" | "sleep" | "shutdown"
    pub after_queue: String,
    pub render_break: bool,
    pub organize_exports: bool,
    pub watch_dirs: Vec<String>,
    pub export_root: String,

    // --- tugas otomatis & integrasi
    pub auto_track: bool,
    pub git_repos: Vec<GitRepo>,
    pub ics_url: String,
    pub webhook_url: String,
    pub telegram_chat_id: Option<i64>,
    pub telegram_pair_code: String,
    pub stt_base_url: String,
    pub stt_model: String,

    // --- rencana pagi & klien
    pub plan_hour: u32,
    pub last_plan_date: Option<String>,
    pub rates: BTreeMap<String, f64>,
    pub currency: String,
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
            selection_hotkey: "Ctrl+Shift+Space".into(),
            autostart: true,
            report_hour: 22,
            grace_secs: 4,
            default_minutes: 50,
            default_mode: "nudge".into(),
            distractors: default_distractors(),
            first_run_done: false,
            last_report_date: None,
            hud_top: 6,
            hud_monitor: 0,
            allow_keywords: [
                "tutorial", "course", "kursus", "blender", "after effects", "roblox studio", "documentation", "docs",
                "stack overflow", "github", "kuliah", "lecture",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            intent_mode: true,
            flow_protect: true,
            break_every_min: 120,
            hard_block: false,
            block_sites: ["youtube.com", "www.youtube.com", "tiktok.com", "www.tiktok.com", "instagram.com", "www.instagram.com", "x.com", "twitter.com", "reddit.com", "www.reddit.com"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            music: String::new(),
            presets: default_presets(),
            templates: default_templates(),
            blender_path: String::new(),
            aerender_path: String::new(),
            after_queue: "none".into(),
            render_break: true,
            organize_exports: false,
            watch_dirs: Vec::new(),
            export_root: String::new(),
            auto_track: true,
            git_repos: Vec::new(),
            ics_url: String::new(),
            webhook_url: String::new(),
            telegram_chat_id: None,
            telegram_pair_code: String::new(),
            stt_base_url: String::new(),
            stt_model: "whisper-1".into(),
            plan_hour: 9,
            last_plan_date: None,
            rates: BTreeMap::new(),
            currency: "Rp".into(),
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

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

pub fn default_presets() -> Vec<WorkPreset> {
    vec![
        WorkPreset {
            id: "motion".into(),
            name: "Motion (AE)".into(),
            open: strs(&["afterfx"]),
            close_distractors: true,
            minutes: 50,
            mode: "nudge".into(),
            project: None,
        },
        WorkPreset {
            id: "coding".into(),
            name: "Coding".into(),
            open: strs(&["code"]),
            close_distractors: true,
            minutes: 50,
            mode: "nudge".into(),
            project: None,
        },
    ]
}

pub fn default_templates() -> Vec<Template> {
    let t = |name: &str, kw: &[&str], steps: &[&str], est: u32| Template {
        name: name.into(),
        keywords: strs(kw),
        steps: strs(steps),
        est_minutes: Some(est),
    };
    vec![
        t(
            "Video klien",
            &["teaser", "video", "reels", "motion", "edit", "iklan"],
            &[
                "Buka brief & kumpulkan aset klien",
                "Susun urutan shot / storyboard",
                "Bikin animatic kasar",
                "Animasi & polish",
                "Render preview & kirim ke klien",
                "Revisi lalu render final",
            ],
            360,
        ),
        t(
            "Map Roblox",
            &["roblox", "map", "place", "obby"],
            &[
                "Buka place file di Roblox Studio",
                "Blockout layout per zona",
                "Isi aset & lighting",
                "Playtest & catat bug",
                "Publish & update deskripsi",
            ],
            480,
        ),
        t(
            "Model 3D",
            &["sculpt", "model", "maskot", "3d", "karakter"],
            &[
                "Kumpulkan referensi di PureRef",
                "Blockout bentuk dasar",
                "Sculpt detail utama",
                "Retopo & UV",
                "Material & render preview",
            ],
            480,
        ),
        t(
            "Bug fix",
            &["bug", "error", "fix", "benerin", "crash"],
            &[
                "Reproduksi bug di lokal",
                "Cari penyebab lewat log/debugger",
                "Tulis test yang gagal",
                "Perbaiki & jalankan test",
                "Commit & deploy",
            ],
            120,
        ),
        t(
            "Skripsi / tulisan",
            &["skripsi", "bab", "makalah", "paper", "jurnal", "laporan"],
            &[
                "Buka file & catatan revisi",
                "Tulis daftar poin revisi",
                "Kerjakan poin pertama",
                "Cek sitasi & format",
                "Backup & kirim ke dosen",
            ],
            240,
        ),
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
    #[serde(default)]
    pub project: Option<String>,
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
pub struct CommitLog {
    pub repo: String,
    pub project: String,
    pub hash: String,
    pub message: String,
    pub ts: i64,
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
    /// Detik produktif per jam (0..24).
    #[serde(default)]
    pub hours: Vec<u64>,
    /// Detik di pengganggu per jam (0..24).
    #[serde(default)]
    pub hours_dis: Vec<u64>,
    /// Detik tercatat otomatis per tugas (id → detik).
    #[serde(default)]
    pub task_secs: BTreeMap<String, u64>,
    #[serde(default)]
    pub commits: Vec<CommitLog>,
    #[serde(default)]
    pub renders: u32,
    #[serde(default)]
    pub breaks: u32,
    #[serde(default)]
    pub tasks_done: u32,
}

impl DayLog {
    pub fn add_hour(&mut self, hour: usize, distracting: bool) {
        let v = if distracting { &mut self.hours_dis } else { &mut self.hours };
        if v.len() < 24 {
            v.resize(24, 0);
        }
        v[hour.min(23)] += 1;
    }
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
