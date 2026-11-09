//! Loop 1 detik: catat app aktif, deteksi ngelantur, jalankan sesi fokus,
//! pengingat deadline, dan laporan harian.

use crate::store::{Distractor, Store};
use crate::{hud_message, lk, now_ts, to_pill, today_str, win, Shared};
use chrono::{Local, NaiveDate, Timelike};
use serde::Serialize;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const BROWSERS: &[&str] = &[
    "chrome", "msedge", "firefox", "brave", "opera", "vivaldi", "arc", "zen", "floorp", "librewolf", "thorium",
    "chromium", "waterfox", "yandex",
];
const IGNORED: &[&str] = &[
    "lockapp", "searchhost", "searchapp", "shellexperiencehost", "startmenuexperiencehost", "textinputhost",
];
/// Lewat dari ini tanpa input = dianggap tidak di depan laptop (kecuali lagi nonton pengganggu).
const AWAY_SECS: u32 = 180;

#[derive(Clone)]
pub struct Session {
    pub task_id: Option<String>,
    pub task_title: String,
    pub step: Option<String>,
    pub start: i64,
    pub planned_secs: u64,
    pub mode: String,
    pub focus_secs: u64,
    pub distracted_secs: u64,
    pub distractions: u32,
    pub streak: u32,
    pub last_good_hwnd: isize,
    pub nudging: bool,
    pub current: String,
    pub distracted: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct SessionView {
    pub active: bool,
    pub task_id: Option<String>,
    pub task_title: String,
    pub step: Option<String>,
    pub remaining: u64,
    pub planned: u64,
    pub focus_secs: u64,
    pub distracted_secs: u64,
    pub distractions: u32,
    pub mode: String,
    pub distracted: bool,
    pub current: String,
}

impl Session {
    pub fn view(&self, now: i64) -> SessionView {
        let elapsed = (now - self.start).max(0) as u64;
        SessionView {
            active: true,
            task_id: self.task_id.clone(),
            task_title: self.task_title.clone(),
            step: self.step.clone(),
            remaining: self.planned_secs.saturating_sub(elapsed),
            planned: self.planned_secs,
            focus_secs: self.focus_secs,
            distracted_secs: self.distracted_secs,
            distractions: self.distractions,
            mode: self.mode.clone(),
            distracted: self.distracted,
            current: self.current.clone(),
        }
    }
}

pub fn classify(exe: &str, title: &str, distractors: &[Distractor]) -> (String, bool) {
    let is_browser = BROWSERS.contains(&exe);
    let title_l = title.to_lowercase();
    for d in distractors {
        for p in &d.patterns {
            let p = p.trim().to_lowercase();
            if p.is_empty() {
                continue;
            }
            let hit = match p.strip_prefix("exe:") {
                Some(e) => exe == e.trim(),
                None => is_browser && title_l.contains(&p),
            };
            if hit {
                return (d.name.clone(), true);
            }
        }
    }
    (pretty(exe), false)
}

fn pretty(exe: &str) -> String {
    let known = match exe {
        "code" => "VS Code",
        "cursor" => "Cursor",
        "zed" => "Zed",
        "devenv" => "Visual Studio",
        "blender" => "Blender",
        "afterfx" => "After Effects",
        "adobe premiere pro" => "Premiere Pro",
        "photoshop" => "Photoshop",
        "illustrator" => "Illustrator",
        "robloxstudiobeta" => "Roblox Studio",
        "explorer" => "File Explorer",
        "windowsterminal" | "wt" | "cmd" | "powershell" | "pwsh" | "conhost" => "Terminal",
        "claude" => "Claude",
        "chrome" => "Chrome",
        "msedge" => "Edge",
        "firefox" => "Firefox",
        "brave" => "Brave",
        "opera" => "Opera",
        "vivaldi" => "Vivaldi",
        "arc" => "Arc",
        "zen" => "Zen",
        "figma" => "Figma",
        "obs64" => "OBS",
        "notion" => "Notion",
        "obsidian" => "Obsidian",
        "spotify" => "Spotify",
        "resolve" => "DaVinci Resolve",
        "unity" => "Unity",
        "lm studio" => "LM Studio",
        "winword" => "Word",
        "excel" => "Excel",
        "powerpnt" => "PowerPoint",
        "parsecd" => "Parsec",
        _ => "",
    };
    if !known.is_empty() {
        return known.into();
    }
    let mut c = exe.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Lainnya".into(),
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let own = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| "gaspol".into());
        let mut tick: u64 = 0;
        let mut day_streak: u32 = 0;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick += 1;
            step(&app, &own, &mut day_streak);
            let st = app.state::<Shared>();
            if tick % 30 == 0 {
                let d = lk(&st.day);
                st.store.save(&Store::day_name(&d.date), &*d);
            }
            if tick % 60 == 5 {
                check_reminders(&app, &st);
                check_report(&app, &st);
            }
        }
    });
}

fn step(app: &AppHandle, own: &str, day_streak: &mut u32) {
    let st = app.state::<Shared>();
    let (distractors, grace) = {
        let s = lk(&st.settings);
        (s.distractors.clone(), s.grace_secs.clamp(1, 120))
    };
    let now = now_ts();
    let idle = win::idle_secs();

    // Ganti hari → simpan log lama, buka log baru.
    {
        let today = today_str();
        let mut d = lk(&st.day);
        if d.date != today {
            if !d.date.is_empty() {
                st.store.save(&Store::day_name(&d.date), &*d);
            }
            *d = st.store.load(&Store::day_name(&today));
            d.date = today;
        }
    }

    // (label, distracting, hwnd)
    let current = win::foreground().and_then(|fg| {
        if fg.exe.is_empty() || fg.exe == own || IGNORED.contains(&fg.exe.as_str()) {
            return None;
        }
        let (label, dis) = classify(&fg.exe, &fg.title, &distractors);
        Some((label, dis, fg.hwnd))
    });
    let present = |dis: bool| dis || idle < AWAY_SECS;

    // Catatan harian.
    if let Some((label, dis, _)) = &current {
        if present(*dis) {
            let mut d = lk(&st.day);
            let e = d.apps.entry(label.clone()).or_default();
            e.secs += 1;
            e.distracting |= *dis;
            if *dis {
                *day_streak += 1;
                if *day_streak == grace {
                    d.distractions += 1;
                }
            } else {
                *day_streak = 0;
            }
        }
    } else {
        *day_streak = 0;
    }

    // Sesi fokus.
    let mut finished = false;
    let mut view = None;
    let mut hud: Option<(serde_json::Value, bool)> = None;
    let mut minimize = None;
    {
        let mut guard = lk(&st.session);
        if let Some(s) = guard.as_mut() {
            if (now - s.start).max(0) as u64 >= s.planned_secs {
                finished = true;
            } else {
                match &current {
                    Some((label, true, hwnd)) => {
                        s.distracted_secs += 1;
                        s.streak += 1;
                        s.distracted = true;
                        s.current = label.clone();
                        let strict = s.mode == "strict";
                        let threshold = if strict { 2 } else { grace };
                        if s.streak == threshold {
                            s.distractions += 1;
                            if s.mode != "passive" {
                                s.nudging = true;
                                hud = Some((
                                    json!({
                                        "kind": "nudge",
                                        "strict": strict,
                                        "label": label,
                                        "title": s.task_title,
                                        "step": s.step,
                                        "task_id": s.task_id,
                                    }),
                                    true,
                                ));
                            }
                            if strict {
                                minimize = Some(*hwnd);
                            }
                        } else if strict && s.streak > threshold && s.streak % 2 == 0 {
                            minimize = Some(*hwnd);
                        } else if s.mode == "nudge" && !s.nudging && s.streak > threshold && (s.streak - threshold) % 90 == 0 {
                            // Diklik "Sebentar" tapi masih betah di situ → tegur lagi tiap 90 detik.
                            s.nudging = true;
                            hud = Some((
                                json!({
                                    "kind": "nudge",
                                    "strict": false,
                                    "label": label,
                                    "title": s.task_title,
                                    "step": s.step,
                                    "task_id": s.task_id,
                                }),
                                true,
                            ));
                        }
                    }
                    Some((label, false, hwnd)) => {
                        if present(false) {
                            s.focus_secs += 1;
                        }
                        s.streak = 0;
                        s.distracted = false;
                        s.current = label.clone();
                        s.last_good_hwnd = *hwnd;
                        // Mode negur: begitu balik kerja, HUD kembali jadi pil kecil.
                        if s.nudging && s.mode == "nudge" {
                            s.nudging = false;
                            hud = Some((json!({ "kind": "pill" }), false));
                        }
                    }
                    None => {
                        s.streak = 0;
                        s.distracted = false;
                    }
                }
                view = Some(s.view(now));
            }
        }
    }

    if finished {
        crate::finish_session(app, true);
        return;
    }
    if let Some(h) = minimize {
        win::minimize(h);
    }
    if let Some(v) = view {
        let _ = app.emit("session", v);
    }
    if let Some((msg, interactive)) = hud {
        if interactive {
            hud_message(app, msg, true);
        } else {
            to_pill(app);
        }
    }
}

/// Deadline ≤ besok tapi belum disentuh → tegur dan kasih langkah pertama.
fn check_reminders(app: &AppHandle, st: &Shared) {
    if lk(&st.session).is_some() || win::idle_secs() > 300 {
        return;
    }
    let local = Local::now();
    if !(8..23).contains(&local.hour()) {
        return;
    }
    let today = local.date_naive();
    let now = now_ts();
    let pick = {
        let mut tasks = lk(&st.tasks);
        let best = tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.done)
            .filter(|(_, t)| t.last_touched.map_or(true, |x| now - x > 6 * 3600))
            .filter(|(_, t)| t.last_nudge.map_or(true, |x| now - x > 2 * 3600))
            .filter_map(|(i, t)| {
                let dl = NaiveDate::parse_from_str(t.deadline.as_deref()?, "%Y-%m-%d").ok()?;
                let days = (dl - today).num_days();
                (days <= 1).then_some((i, days))
            })
            .min_by_key(|(_, days)| *days);
        best.map(|(i, days)| {
            tasks[i].last_nudge = Some(now);
            let t = tasks[i].clone();
            st.store.save("tasks.json", &*tasks);
            (t, days)
        })
    };
    if let Some((t, days)) = pick {
        let when = match days {
            d if d < 0 => format!("telat {} hari", -d),
            0 => "hari ini".to_string(),
            _ => "besok".to_string(),
        };
        let step = t.steps.iter().find(|s| !s.done).map(|s| s.text.clone());
        hud_message(
            app,
            json!({
                "kind": "remind",
                "task_id": t.id,
                "title": t.title,
                "when": when,
                "step": step,
                "snoozes": t.snoozes,
            }),
            true,
        );
    }
}

fn check_report(app: &AppHandle, st: &Shared) {
    if lk(&st.session).is_some() {
        return;
    }
    let today = today_str();
    {
        let s = lk(&st.settings);
        if Local::now().hour() < s.report_hour || s.last_report_date.as_deref() == Some(today.as_str()) {
            return;
        }
    }
    let summary = {
        let d = lk(&st.day);
        let total: u64 = d.apps.values().map(|a| a.secs).sum();
        if total < 600 {
            return;
        }
        let distracted: u64 = d.apps.values().filter(|a| a.distracting).map(|a| a.secs).sum();
        let focus: u64 = d.sessions.iter().map(|s| s.focus_secs).sum();
        let top = d
            .apps
            .iter()
            .filter(|(_, a)| !a.distracting)
            .max_by_key(|(_, a)| a.secs)
            .map(|(n, _)| n.clone());
        json!({
            "kind": "report",
            "total": total,
            "distracted": distracted,
            "focus": focus,
            "distractions": d.distractions,
            "top": top,
        })
    };
    {
        let mut s = lk(&st.settings);
        s.last_report_date = Some(today);
        st.store.save("settings.json", &*s);
    }
    hud_message(app, summary, true);
}
