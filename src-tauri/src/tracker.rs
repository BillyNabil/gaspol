//! Loop 1 detik: catat app aktif, deteksi ngelantur (dengan konteks), jalankan sesi fokus,
//! pencatatan otomatis per tugas, pengingat, istirahat, rencana pagi, dan laporan harian.

use crate::store::{Distractor, Store, Task};
use crate::{hud_message, insights, lk, now_ts, render, to_pill, today_str, win, Shared};
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
/// Kata umum yang tidak cukup untuk mencocokkan jendela ke tugas.
const GENERIC: &[&str] = &[
    "render", "video", "final", "design", "desain", "model", "update", "client", "klien", "revisi", "tugas", "file",
    "edit", "project", "proyek", "untuk", "dengan", "yang", "bikin", "buat", "kerjain", "selesaikan", "google",
    "chrome", "microsoft", "visual", "studio", "code", "window", "untitled", "document", "new", "tab", "blender",
    "after", "effects", "roblox", "word", "excel",
];

pub fn is_browser(exe: &str) -> bool {
    BROWSERS.contains(&exe)
}

#[derive(Clone, Default)]
pub struct Session {
    pub task_id: Option<String>,
    pub task_title: String,
    pub project: Option<String>,
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
    /// App/pola sah untuk tugas ini (#7).
    pub allowed: Vec<String>,
    /// Kata dari judul tugas — tab yang relevan dengan tugas tidak dihitung ngelantur.
    pub keywords: Vec<String>,
    /// "Pasang niat": boleh di pengganggu sampai jam ini (#8).
    pub allow_until: i64,
    pub allow_label: String,
    pub allowed_total: u64,
    pub offtask: u32,
    pub offtask_warned: bool,
    pub last_dis_ts: i64,
    pub flow_ext: u64,
}

#[derive(Serialize, Clone, Default)]
pub struct SessionView {
    pub active: bool,
    pub task_id: Option<String>,
    pub task_title: String,
    pub project: Option<String>,
    pub step: Option<String>,
    pub remaining: u64,
    pub planned: u64,
    pub focus_secs: u64,
    pub distracted_secs: u64,
    pub distractions: u32,
    pub mode: String,
    pub distracted: bool,
    pub current: String,
    pub allow_left: i64,
    pub allow_label: String,
    pub flow_ext: u64,
}

impl Session {
    pub fn view(&self, now: i64) -> SessionView {
        let elapsed = (now - self.start).max(0) as u64;
        SessionView {
            active: true,
            task_id: self.task_id.clone(),
            task_title: self.task_title.clone(),
            project: self.project.clone(),
            step: self.step.clone(),
            remaining: self.planned_secs.saturating_sub(elapsed),
            planned: self.planned_secs,
            focus_secs: self.focus_secs,
            distracted_secs: self.distracted_secs,
            distractions: self.distractions,
            mode: self.mode.clone(),
            distracted: self.distracted,
            current: self.current.clone(),
            allow_left: (self.allow_until - now).max(0),
            allow_label: self.allow_label.clone(),
            flow_ext: self.flow_ext,
        }
    }
}

pub struct Class {
    pub label: String,
    pub distracting: bool,
}

pub fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(String::from)
        .collect()
}

/// `allow`: kata yang membuat tab pengganggu tetap "sah" (tutorial, docs, ...).
/// `task_words`: kata dari tugas yang sedang dikerjakan.
pub fn classify(exe: &str, title: &str, distractors: &[Distractor], allow: &[String], task_words: &[String]) -> Class {
    let browser = is_browser(exe);
    let title_l = title.to_lowercase();
    for d in distractors {
        for p in &d.patterns {
            let p = p.trim().to_lowercase();
            if p.is_empty() {
                continue;
            }
            let hit = match p.strip_prefix("exe:") {
                Some(e) => exe == e.trim(),
                None => browser && title_l.contains(&p),
            };
            if !hit {
                continue;
            }
            // Konteks: tutorial Blender di YouTube ≠ nonton random.
            if browser {
                let ok_kw = allow.iter().map(|a| a.trim().to_lowercase()).find(|a| !a.is_empty() && title_l.contains(a.as_str()));
                let ok_task = task_words.iter().find(|w| w.len() >= 5 && title_l.contains(w.as_str()));
                if ok_kw.is_some() || ok_task.is_some() {
                    return Class { label: format!("{} · relevan", d.name), distracting: false };
                }
            }
            return Class { label: d.name.clone(), distracting: true };
        }
    }
    Class { label: pretty(exe), distracting: false }
}

pub fn matches_allowed(exe: &str, title: &str, allowed: &[String]) -> bool {
    let t = title.to_lowercase();
    allowed.iter().any(|a| {
        let a = a.trim().to_lowercase();
        if a.is_empty() {
            return false;
        }
        match a.strip_prefix("exe:") {
            Some(e) => exe == e.trim(),
            None => exe == a || t.contains(&a),
        }
    })
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
        "aerender" => "aerender",
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

/// Jendela aktif cocok dengan tugas mana? (#14)
fn match_task(tasks: &[Task], title: &str) -> Option<String> {
    let tw = words(title);
    if tw.is_empty() {
        return None;
    }
    let mut best: Option<(usize, String)> = None;
    for t in tasks.iter().filter(|t| !t.done) {
        let mut kw = words(&t.title);
        if let Some(p) = &t.project {
            kw.extend(words(p));
        }
        kw.retain(|w| !GENERIC.contains(&w.as_str()));
        kw.dedup();
        let hits: Vec<&String> = kw.iter().filter(|w| tw.contains(w)).collect();
        let strong = hits.iter().any(|w| w.chars().count() >= 6);
        let score = hits.len();
        if score >= 2 || (score == 1 && strong) {
            if best.as_ref().map_or(true, |b| score > b.0) {
                best = Some((score, t.id.clone()));
            }
        }
    }
    best.map(|b| b.1)
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let own = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| "gaspol".into());
        let mut tick: u64 = 0;
        let mut day_streak: u32 = 0;
        let mut pending_auto: u32 = 0;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick += 1;
            step(&app, &own, &mut day_streak, &mut pending_auto, tick);
            let st = app.state::<Shared>();
            if tick % 5 == 0 {
                render::tick_external(&app);
            }
            if tick % 30 == 0 {
                let d = lk(&st.day);
                st.store.save(&Store::day_name(&d.date), &*d);
            }
            if tick % 60 == 5 {
                check_reminders(&app, &st);
                check_plan(&app, &st);
                check_report(&app, &st);
            }
        }
    });
}

fn step(app: &AppHandle, own: &str, day_streak: &mut u32, pending_auto: &mut u32, tick: u64) {
    let st = app.state::<Shared>();
    let (distractors, allow, grace, intent_mode, flow_protect, break_every, auto_track) = {
        let s = lk(&st.settings);
        (
            s.distractors.clone(),
            s.allow_keywords.clone(),
            s.grace_secs.clamp(1, 120),
            s.intent_mode,
            s.flow_protect,
            s.break_every_min,
            s.auto_track,
        )
    };
    let now = now_ts();
    let idle = win::idle_secs();
    let hour = Local::now().hour() as usize;

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

    let session_words = lk(&st.session).as_ref().map(|s| s.keywords.clone()).unwrap_or_default();

    // (class, hwnd, exe, title)
    let current = win::foreground().and_then(|fg| {
        if fg.exe.is_empty() || fg.exe == own || IGNORED.contains(&fg.exe.as_str()) {
            return None;
        }
        let c = classify(&fg.exe, &fg.title, &distractors, &allow, &session_words);
        Some((c, fg.hwnd, fg.exe, fg.title))
    });
    let present = |dis: bool| dis || idle < AWAY_SECS;
    let on_render_break = render::on_break(app);

    // Catatan harian + jam produktif.
    if let Some((c, _, _, _)) = &current {
        if present(c.distracting) {
            let mut d = lk(&st.day);
            let e = d.apps.entry(c.label.clone()).or_default();
            e.secs += 1;
            e.distracting |= c.distracting;
            d.add_hour(hour, c.distracting);
            if c.distracting {
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

    // Istirahat (#9): aktif nonstop terlalu lama → ingatkan minum & peregangan.
    let mut break_hud = false;
    {
        let mut rt = lk(&st.rt);
        if idle < 60 {
            rt.active_streak += 1;
            rt.idle_run = 0;
        } else {
            rt.idle_run += 1;
            if rt.idle_run >= 300 {
                rt.active_streak = 0;
            }
        }
        let on_distractor = current.as_ref().map(|c| c.0.distracting).unwrap_or(false);
        if break_every > 0 && rt.active_streak >= break_every * 60 && !on_distractor {
            rt.active_streak = 0;
            break_hud = true;
        }
    }
    if break_hud {
        lk(&st.day).breaks += 1;
        hud_message(app, json!({ "kind": "break", "mins": break_every }), true);
    }

    let session_active = lk(&st.session).is_some();

    // Pencatatan otomatis per tugas tanpa sesi (#14).
    if auto_track && !session_active {
        if let Some((c, _, _, title)) = &current {
            if !c.distracting && idle < AWAY_SECS {
                if tick % 10 == 0 || lk(&st.rt).auto_task.is_none() {
                    let found = match_task(&lk(&st.tasks), title);
                    let mut rt = lk(&st.rt);
                    rt.auto_task = found.map(|id| (id, 0));
                }
                let id = lk(&st.rt).auto_task.as_ref().map(|a| a.0.clone());
                if let Some(id) = id {
                    *lk(&st.day).task_secs.entry(id.clone()).or_default() += 1;
                    *pending_auto += 1;
                    if *pending_auto >= 60 {
                        let add = std::mem::take(pending_auto) as u64;
                        let mut tasks = lk(&st.tasks);
                        if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                            t.auto_secs += add;
                            t.last_touched = Some(now);
                        }
                        st.store.save("tasks.json", &*tasks);
                        drop(tasks);
                        crate::emit_tasks(app);
                    }
                }
            }
        }
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
                // Lagi flow → perpanjang 15 menit diam-diam (maks 1 jam).
                let flowing = flow_protect
                    && s.flow_ext < 3600
                    && now - s.last_dis_ts > 600
                    && idle < 60
                    && current.as_ref().map(|c| !c.0.distracting).unwrap_or(false);
                if flowing {
                    s.planned_secs += 900;
                    s.flow_ext += 900;
                } else {
                    finished = true;
                }
            }
            if !finished {
                let nudge_msg = |s: &Session, label: &str, strict: bool| {
                    json!({
                        "kind": "nudge",
                        "strict": strict,
                        "label": label,
                        "title": s.task_title,
                        "step": s.step,
                        "task_id": s.task_id,
                    })
                };
                match &current {
                    Some((c, hwnd, _, _)) if c.distracting => {
                        s.distracted_secs += 1;
                        s.distracted = true;
                        s.current = c.label.clone();
                        s.last_dis_ts = now;
                        s.offtask = 0;
                        let allowed_now = now < s.allow_until;
                        if allowed_now {
                            s.allowed_total += 1;
                        } else if !on_render_break {
                            s.streak += 1;
                            let strict = s.mode == "strict";
                            let threshold = if strict { 2 } else { grace };
                            if s.streak == threshold {
                                s.distractions += 1;
                                if s.mode != "passive" {
                                    s.nudging = true;
                                    // Pasang niat dulu (maks 20 menit izin per sesi).
                                    if !strict && intent_mode && s.allowed_total < 1200 && s.allow_until == 0 {
                                        hud = Some((
                                            json!({ "kind": "intent", "label": c.label, "title": s.task_title, "task_id": s.task_id }),
                                            true,
                                        ));
                                    } else {
                                        hud = Some((nudge_msg(s, &c.label, strict), true));
                                    }
                                }
                                if strict {
                                    minimize = Some(*hwnd);
                                }
                            } else if strict && s.streak > threshold && s.streak % 2 == 0 {
                                minimize = Some(*hwnd);
                            } else if s.mode == "nudge" && !s.nudging && s.streak > threshold && (s.streak - threshold) % 90 == 0 {
                                s.nudging = true;
                                hud = Some((nudge_msg(s, &c.label, false), true));
                            }
                        }
                    }
                    Some((c, hwnd, exe, title)) => {
                        if present(false) {
                            s.focus_secs += 1;
                        }
                        s.streak = 0;
                        s.distracted = false;
                        s.current = c.label.clone();
                        s.last_good_hwnd = *hwnd;
                        // Izin habis dan sudah balik → izin berikutnya harus diminta lagi.
                        if s.allow_until != 0 && now >= s.allow_until {
                            s.allow_until = 0;
                            s.allow_label.clear();
                        }
                        // App di luar daftar sah (#7) → teguran lembut setelah 90 detik.
                        if !s.allowed.is_empty() && s.mode == "nudge" {
                            if matches_allowed(exe, title, &s.allowed) || is_browser(exe) {
                                s.offtask = 0;
                                s.offtask_warned = false;
                            } else if idle < 60 {
                                s.offtask += 1;
                                if s.offtask == 90 && !s.offtask_warned {
                                    s.offtask_warned = true;
                                    hud = Some((
                                        json!({ "kind": "offtask", "label": c.label, "title": s.task_title, "task_id": s.task_id }),
                                        true,
                                    ));
                                }
                            }
                        }
                        if s.nudging && s.mode == "nudge" && hud.is_none() {
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

/// Deadline mepet (dikoreksi dengan kebiasaan meleset estimasi) tapi belum disentuh → tagih.
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
    let ratio = insights::estimate_ratio(&lk(&st.tasks));
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
                let need_min = insights::remaining_minutes(t, ratio);
                // ±3 jam fokus efektif per hari.
                let tight = need_min.map(|m| m as f64 / 60.0 > (days.max(0) as f64 + 1.0) * 3.0).unwrap_or(false);
                (days <= 1 || tight).then_some((i, days, need_min))
            })
            .min_by_key(|(_, days, _)| *days);
        best.map(|(i, days, need)| {
            tasks[i].last_nudge = Some(now);
            let t = tasks[i].clone();
            st.store.save("tasks.json", &*tasks);
            (t, days, need)
        })
    };
    if let Some((t, days, need)) = pick {
        let when = match days {
            d if d < 0 => format!("telat {} hari", -d),
            0 => "hari ini".to_string(),
            1 => "besok".to_string(),
            d => format!("{d} hari lagi"),
        };
        let step = t.steps.iter().find(|s| !s.done).map(|s| s.text.clone());
        crate::notify_remote(app, &format!("⏰ Deadline {when}: {}{}", t.title, step.as_deref().map(|s| format!("\n→ {s}")).unwrap_or_default()));
        hud_message(
            app,
            json!({
                "kind": "remind",
                "task_id": t.id,
                "title": t.title,
                "when": when,
                "step": step,
                "snoozes": t.snoozes,
                "need_min": need,
            }),
            true,
        );
    }
}

/// Rencana pagi (#22): sekali sehari di jam rencana, saat kamu sudah di depan laptop.
fn check_plan(app: &AppHandle, st: &Shared) {
    if lk(&st.session).is_some() || win::idle_secs() > 120 {
        return;
    }
    let today = today_str();
    {
        let s = lk(&st.settings);
        let h = Local::now().hour();
        if h < s.plan_hour || h >= 13 || s.last_plan_date.as_deref() == Some(today.as_str()) {
            return;
        }
    }
    let plan = insights::plan(app);
    if plan.tasks.is_empty() {
        return;
    }
    {
        let mut s = lk(&st.settings);
        s.last_plan_date = Some(today);
        st.store.save("settings.json", &*s);
    }
    hud_message(app, json!({ "kind": "plan", "tasks": plan.tasks, "free": plan.free_text }), true);
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
    crate::automation::share_daily(app, insights::daily_text(app));
    hud_message(app, summary, true);
}
