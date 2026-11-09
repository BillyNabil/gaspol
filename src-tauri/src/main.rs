#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod automation;
mod insights;
mod render;
mod store;
mod tracker;
mod win;

pub use automation::notify_remote;
use automation::Runtime;
use base64::Engine;
use render::{RenderJob, RenderState, RenderView};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};
use store::{DayLog, SessionLog, Settings, Step, Store, Task};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracker::{Session, SessionView};

pub struct Shared {
    pub store: Store,
    pub tasks: Mutex<Vec<Task>>,
    pub settings: Mutex<Settings>,
    pub session: Mutex<Option<Session>>,
    pub day: Mutex<DayLog>,
    pub hotkey_error: Mutex<Option<String>>,
    pub render: Mutex<RenderState>,
    pub rt: Mutex<Runtime>,
}

impl Shared {
    fn load(dir: std::path::PathBuf) -> Self {
        let store = Store::new(dir);
        let tasks: Vec<Task> = store.load("tasks.json");
        let settings: Settings = store.load("settings.json");
        let today = today_str();
        let mut day: DayLog = store.load(&Store::day_name(&today));
        day.date = today;
        Self {
            store,
            tasks: Mutex::new(tasks),
            settings: Mutex::new(settings),
            session: Mutex::new(None),
            day: Mutex::new(day),
            hotkey_error: Mutex::new(None),
            render: Mutex::new(RenderState::default()),
            rt: Mutex::new(Runtime::default()),
        }
    }
}

// ------------------------------------------------------------------ helpers

pub fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn now_ts() -> i64 {
    chrono::Local::now().timestamp()
}

pub fn today_str() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!("t{:x}{:x}", chrono::Local::now().timestamp_millis(), N.fetch_add(1, Ordering::Relaxed))
}

const SECRET_KINDS: &[&str] = &["claude", "local", "custom", "telegram", "stt"];

fn secret_entry(kind: &str) -> Option<keyring::Entry> {
    SECRET_KINDS.contains(&kind).then(|| keyring::Entry::new("gaspol", kind).ok())?
}

pub fn secret_get(kind: &str) -> Option<String> {
    secret_entry(kind)?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn emit_tasks(app: &AppHandle) {
    let st = app.state::<Shared>();
    let tasks = lk(&st.tasks).clone();
    let _ = app.emit("tasks", tasks);
}

/// Project yang sedang dikerjakan: dari sesi aktif, atau tugas yang terakhir disentuh.
pub fn current_project(app: &AppHandle) -> Option<String> {
    let st = app.state::<Shared>();
    if let Some(p) = lk(&st.session).as_ref().and_then(|s| s.project.clone()) {
        return Some(p);
    }
    let p = lk(&st.tasks)
        .iter()
        .filter(|t| !t.done && t.project.is_some())
        .max_by_key(|t| t.last_touched.unwrap_or(0))
        .and_then(|t| t.project.clone());
    p
}

fn place(w: &WebviewWindow, top_ratio: f64, top_px: f64, monitor: u32) {
    let mut mons = w.available_monitors().unwrap_or_default();
    mons.sort_by_key(|m| (m.position().x, m.position().y));
    let m = match monitor {
        0 => w.primary_monitor().ok().flatten(),
        n => mons.get(n as usize - 1).cloned().or_else(|| w.primary_monitor().ok().flatten()),
    };
    let Some(m) = m else { return };
    let size = w.outer_size().unwrap_or_default();
    let (mp, ms) = (m.position(), m.size());
    let x = mp.x + (ms.width as i32 - size.width as i32) / 2;
    let y = mp.y + (ms.height as f64 * top_ratio + top_px * m.scale_factor()) as i32;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// Pasang ulang alpha per-piksel tiap kali tampil (lihat `win::ensure_transparent`).
fn transparent(w: &WebviewWindow) {
    if let Ok(h) = w.hwnd() {
        win::ensure_transparent(h.0 as isize);
    }
}

pub fn hud_show(app: &AppHandle, interactive: bool) {
    if let Some(w) = app.get_webview_window("hud") {
        let st = app.state::<Shared>();
        let (top, mon) = {
            let s = lk(&st.settings);
            (s.hud_top.min(400), s.hud_monitor)
        };
        place(&w, 0.0, top as f64, mon);
        let _ = w.set_ignore_cursor_events(!interactive);
        let _ = w.show();
        transparent(&w);
    }
}

pub fn hud_hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("hud") {
        let _ = w.hide();
    }
}

pub fn hud_message(app: &AppHandle, msg: Value, interactive: bool) {
    let _ = app.emit("hud", msg);
    hud_show(app, interactive);
}

/// Kembalikan HUD ke pil timer (kalau ada sesi / render), atau sembunyikan.
pub fn to_pill(app: &AppHandle) {
    let st = app.state::<Shared>();
    let session = lk(&st.session).is_some();
    let rendering = lk(&st.render).pid.is_some();
    let active = session || rendering;
    if active {
        let _ = app.emit("hud", json!({ "kind": "pill" }));
        hud_show(app, false);
    } else {
        hud_hide(app);
    }
}

/// Jendela utama dibuat saat dibuka dan dihancurkan saat ditutup,
/// jadi selama di tray tidak ada renderer dashboard yang makan RAM.
fn show_main(app: &AppHandle, tab: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(t) = tab {
            let _ = app.emit("nav", t);
        }
        return;
    }
    let Some(mut cfg) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else { return };
    cfg.create = true;
    cfg.visible = true;
    if let Some(t) = tab {
        cfg.url = tauri::WebviewUrl::App(format!("index.html#{t}").into());
    }
    if let Ok(w) = tauri::WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
        let _ = w.set_focus();
    }
}

fn open_capture(app: &AppHandle, prefill: Option<String>) {
    let Some(w) = app.get_webview_window("capture") else { return };
    let mon = lk(&app.state::<Shared>().settings).hud_monitor;
    place(&w, 0.2, 0.0, mon);
    let _ = w.show();
    transparent(&w);
    let _ = w.set_focus();
    let _ = app.emit("capture-open", prefill);
}

fn toggle_capture(app: &AppHandle) {
    let Some(w) = app.get_webview_window("capture") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    } else {
        open_capture(app, None);
    }
}

fn register_hotkeys(app: &AppHandle, main: &str, selection: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(main).map_err(|e| format!("Hotkey \"{main}\" gagal dipasang: {e}"))?;
    let sel = selection.trim();
    if !sel.is_empty() && !sel.eq_ignore_ascii_case(main) {
        gs.register(sel).map_err(|e| format!("Hotkey \"{sel}\" gagal dipasang: {e}"))?;
    }
    Ok(())
}

pub fn finish_session(app: &AppHandle, completed: bool) {
    let st = app.state::<Shared>();
    let Some(s) = lk(&st.session).take() else { return };
    let now = now_ts();
    {
        let mut d = lk(&st.day);
        d.sessions.push(SessionLog {
            task_id: s.task_id.clone(),
            task_title: s.task_title.clone(),
            project: s.project.clone(),
            start: s.start,
            end: now,
            planned_secs: s.planned_secs,
            focus_secs: s.focus_secs,
            distracted_secs: s.distracted_secs,
            distractions: s.distractions,
            mode: s.mode.clone(),
            completed,
        });
        let name = Store::day_name(&d.date);
        st.store.save(&name, &*d);
    }
    if let Some(id) = &s.task_id {
        let mut tasks = lk(&st.tasks);
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id) {
            t.focus_secs += s.focus_secs;
            t.last_touched = Some(now);
        }
        st.store.save("tasks.json", &*tasks);
    }
    automation::on_session_end(app);
    emit_tasks(app);
    let _ = app.emit("session", SessionView::default());
    if completed {
        hud_message(
            app,
            json!({
                "kind": "done",
                "task_id": s.task_id,
                "title": s.task_title,
                "focus": s.focus_secs,
                "distracted": s.distracted_secs,
                "distractions": s.distractions,
                "flow": s.flow_ext,
            }),
            true,
        );
    } else {
        to_pill(app);
    }
}

pub fn start_session_inner(app: &AppHandle, task_id: Option<String>, minutes: Option<u32>, mode: Option<String>) -> SessionView {
    let st = app.state::<Shared>();
    if lk(&st.session).is_some() {
        finish_session(app, false);
    }
    let (def_min, def_mode) = {
        let s = lk(&st.settings);
        (s.default_minutes, s.default_mode.clone())
    };
    let minutes = minutes.unwrap_or(def_min).clamp(1, 240);
    let mode = mode
        .filter(|m| matches!(m.as_str(), "passive" | "nudge" | "strict"))
        .unwrap_or(def_mode);
    let now = now_ts();
    let found = {
        let mut tasks = lk(&st.tasks);
        let found = task_id.as_ref().and_then(|id| tasks.iter_mut().find(|t| &t.id == id)).map(|t| {
            t.last_touched = Some(now);
            t.clone()
        });
        st.store.save("tasks.json", &*tasks);
        found
    };
    let s = match &found {
        Some(t) => Session {
            task_id: Some(t.id.clone()),
            task_title: t.title.clone(),
            project: t.project.clone(),
            step: t.steps.iter().find(|s| !s.done).map(|s| s.text.clone()),
            allowed: t.allowed.clone(),
            keywords: tracker::words(&format!("{} {}", t.title, t.project.clone().unwrap_or_default())),
            ..Default::default()
        },
        None => Session { task_title: "Sesi fokus bebas".into(), ..Default::default() },
    };
    let s = Session { start: now, planned_secs: minutes as u64 * 60, mode: mode.clone(), ..s };
    let view = s.view(now);
    *lk(&st.session) = Some(s);
    automation::on_session_start(app, &mode);
    let _ = app.emit("session", view.clone());
    to_pill(app);
    view
}

// ------------------------------------------------------------------ commands

#[derive(Serialize)]
struct StateView {
    tasks: Vec<Task>,
    settings: Settings,
    session: SessionView,
    has_claude_key: bool,
    has_local_key: bool,
    has_custom_key: bool,
    has_telegram_key: bool,
    has_stt_key: bool,
    telegram_ok: bool,
    telegram_error: Option<String>,
    cal_error: Option<String>,
    cal_count: usize,
    hotkey_error: Option<String>,
    data_dir: String,
    days: Vec<String>,
    stats: insights::Stats,
    render: RenderView,
    monitors: Vec<String>,
    version: String,
}

#[tauri::command]
fn get_state(app: AppHandle, st: State<'_, Shared>) -> StateView {
    let session = lk(&st.session).as_ref().map(|s| s.view(now_ts())).unwrap_or_default();
    let (telegram_ok, telegram_error, cal_error, cal_count) = {
        let rt = lk(&st.rt);
        (rt.telegram_ok, rt.telegram_error.clone(), rt.cal_error.clone(), rt.cal.len())
    };
    // Kumpulkan dulu satu per satu: guard sementara di dalam struct literal hidup sampai
    // akhir statement, dan stats()/render::view() mengunci mutex yang sama → deadlock.
    let tasks = lk(&st.tasks).clone();
    let settings = lk(&st.settings).clone();
    let hotkey_error = lk(&st.hotkey_error).clone();
    let stats = insights::stats(&app);
    let render = render::view(&app);
    let monitors = list_monitors(app.clone());
    StateView {
        tasks,
        settings,
        session,
        has_claude_key: secret_get("claude").is_some(),
        has_local_key: secret_get("local").is_some(),
        has_custom_key: secret_get("custom").is_some(),
        has_telegram_key: secret_get("telegram").is_some(),
        has_stt_key: secret_get("stt").is_some(),
        telegram_ok,
        telegram_error,
        cal_error,
        cal_count,
        hotkey_error,
        data_dir: st.store.dir.display().to_string(),
        days: st.store.list_days(),
        stats,
        render,
        monitors,
        version: app.package_info().version.to_string(),
    }
}

#[derive(Serialize)]
pub struct CaptureResult {
    pub task: Task,
    pub warning: Option<String>,
}

fn store_new_task(app: &AppHandle, draft: ai::Draft, raw: String, ai_used: bool, template: Option<String>) -> Task {
    let task = Task {
        id: new_id(),
        title: draft.title,
        project: draft.project,
        deadline: draft.deadline,
        est_minutes: draft.est_minutes,
        steps: draft.steps.into_iter().map(|text| Step { text, done: false }).collect(),
        created_at: now_ts(),
        raw,
        ai: ai_used,
        template,
        ..Default::default()
    };
    {
        let st = app.state::<Shared>();
        let mut tasks = lk(&st.tasks);
        tasks.push(task.clone());
        st.store.save("tasks.json", &*tasks);
    }
    emit_tasks(app);
    task
}

/// Satu kalimat → tugas (dipakai capture bar, dashboard, dan bot Telegram).
pub async fn capture_text(app: &AppHandle, text: String) -> Result<CaptureResult, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Tulis dulu tugasnya".into());
    }
    let settings = lk(&app.state::<Shared>().settings).clone();
    let tpl = ai::find_template(&settings.templates, &text).cloned();
    let multiline = text.lines().filter(|l| !l.trim().is_empty()).count() >= 3;
    let (draft, ai_used, warning) = if settings.ai_provider == "none" || multiline {
        (ai::heuristic_tpl(&text, tpl.as_ref()), false, None)
    } else {
        let key = secret_get(&settings.ai_provider);
        match ai::draft_with(&settings, key, &text, tpl.as_ref()).await {
            Ok(mut d) => {
                if let Some(t) = &tpl {
                    d.est_minutes = d.est_minutes.or(t.est_minutes);
                }
                (d, true, None)
            }
            Err(e) => (ai::heuristic_tpl(&text, tpl.as_ref()), false, Some(e)),
        }
    };
    let task = store_new_task(app, draft, text, ai_used, tpl.map(|t| t.name));
    Ok(CaptureResult { task, warning })
}

#[tauri::command]
async fn capture_task(app: AppHandle, text: String) -> Result<CaptureResult, String> {
    capture_text(&app, text).await
}

#[tauri::command]
async fn capture_image(app: AppHandle, b64: String, mime: String, note: String) -> Result<CaptureResult, String> {
    let settings = lk(&app.state::<Shared>().settings).clone();
    if settings.ai_provider == "none" {
        return Err("Butuh AI (yang bisa baca gambar) untuk mengubah screenshot jadi tugas".into());
    }
    let b64 = b64.split(',').last().unwrap_or("").to_string();
    let draft = ai::draft_image(&settings, secret_get(&settings.ai_provider), &b64, &mime, &note).await?;
    let task = store_new_task(&app, draft, format!("[gambar] {note}"), true, None);
    Ok(CaptureResult { task, warning: None })
}

#[tauri::command]
async fn transcribe(app: AppHandle, b64: String, mime: String) -> Result<String, String> {
    let settings = lk(&app.state::<Shared>().settings).clone();
    let audio = base64::engine::general_purpose::STANDARD
        .decode(b64.split(',').last().unwrap_or(""))
        .map_err(|e| e.to_string())?;
    if audio.len() < 1000 {
        return Err("Rekaman terlalu pendek".into());
    }
    let key = secret_get("stt").or_else(|| {
        (settings.stt_base_url.trim().is_empty() && settings.ai_provider == "custom").then(|| secret_get("custom")).flatten()
    });
    ai::transcribe(&settings, key, audio, &mime).await
}

fn with_task(app: &AppHandle, id: &str, f: impl FnOnce(&mut Task)) -> Result<(), String> {
    let st = app.state::<Shared>();
    {
        let mut tasks = lk(&st.tasks);
        let t = tasks.iter_mut().find(|t| t.id == id).ok_or("Tugas tidak ditemukan")?;
        f(t);
        st.store.save("tasks.json", &*tasks);
    }
    emit_tasks(app);
    Ok(())
}

#[tauri::command]
fn update_task(app: AppHandle, task: Task) -> Result<(), String> {
    let id = task.id.clone();
    with_task(&app, &id, |t| {
        t.title = task.title;
        t.project = task.project.filter(|p| !p.trim().is_empty());
        t.deadline = task.deadline.filter(|d| !d.is_empty());
        t.est_minutes = task.est_minutes.filter(|m| *m > 0);
        t.steps = task.steps;
        t.allowed = task.allowed.into_iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
        t.last_nudge = None;
    })
}

#[tauri::command]
fn delete_task(app: AppHandle, id: String) {
    let st = app.state::<Shared>();
    {
        let mut tasks = lk(&st.tasks);
        tasks.retain(|t| t.id != id);
        st.store.save("tasks.json", &*tasks);
    }
    emit_tasks(&app);
}

#[tauri::command]
fn toggle_step(app: AppHandle, id: String, index: usize) -> Result<(), String> {
    let mut next_step = None;
    with_task(&app, &id, |t| {
        if let Some(s) = t.steps.get_mut(index) {
            s.done = !s.done;
        }
        t.last_touched = Some(now_ts());
        next_step = t.steps.iter().find(|s| !s.done).map(|s| s.text.clone());
    })?;
    // Langkah di sesi yang sedang jalan ikut maju.
    let st = app.state::<Shared>();
    if let Some(s) = lk(&st.session).as_mut().filter(|s| s.task_id.as_deref() == Some(id.as_str())) {
        s.step = next_step;
    }
    Ok(())
}

#[tauri::command]
fn complete_task(app: AppHandle, id: String, done: bool) -> Result<(), String> {
    let mut changed = false;
    with_task(&app, &id, |t| {
        changed = t.done != done;
        t.done = done;
        t.done_at = done.then(now_ts);
    })?;
    if changed && done {
        lk(&app.state::<Shared>().day).tasks_done += 1;
    }
    Ok(())
}

#[tauri::command]
async fn regenerate_steps(app: AppHandle, id: String) -> Result<(), String> {
    let settings = lk(&app.state::<Shared>().settings).clone();
    if settings.ai_provider == "none" {
        return Err("Aktifkan AI dulu di Pengaturan".into());
    }
    let task = lk(&app.state::<Shared>().tasks)
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or("Tugas tidak ditemukan")?;
    let mut prompt = format!("Tugas: {}", task.title);
    if let Some(p) = &task.project {
        prompt += &format!(" (project: {p})");
    }
    if let Some(d) = &task.deadline {
        prompt += &format!(", deadline {d}");
    }
    let done: Vec<&str> = task.steps.iter().filter(|s| s.done).map(|s| s.text.as_str()).collect();
    if !done.is_empty() {
        prompt += &format!(". Sudah selesai: {}. Beri langkah lanjutan yang lebih kecil.", done.join("; "));
    } else {
        prompt += ". Pecah jadi langkah yang lebih kecil lagi.";
    }
    let draft = ai::draft(&settings, secret_get(&settings.ai_provider), &prompt).await?;
    with_task(&app, &id, |t| {
        t.steps.retain(|s| s.done);
        t.steps.extend(draft.steps.into_iter().map(|text| Step { text, done: false }));
    })
}

#[tauri::command]
fn start_session(app: AppHandle, task_id: Option<String>, minutes: Option<u32>, mode: Option<String>) -> SessionView {
    start_session_inner(&app, task_id, minutes, mode)
}

#[tauri::command]
fn stop_session(app: AppHandle) {
    finish_session(&app, false);
}

#[tauri::command]
fn get_day(st: State<'_, Shared>, date: String) -> DayLog {
    let current = lk(&st.day);
    if current.date == date {
        return current.clone();
    }
    drop(current);
    let mut d: DayLog = st.store.load(&Store::day_name(&date));
    d.date = date;
    d
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let st = app.state::<Shared>();
    let old = lk(&st.settings).clone();
    if settings.hotkey != old.hotkey || settings.selection_hotkey != old.selection_hotkey {
        if let Err(e) = register_hotkeys(&app, &settings.hotkey, &settings.selection_hotkey) {
            let _ = register_hotkeys(&app, &old.hotkey, &old.selection_hotkey);
            return Err(e);
        }
        *lk(&st.hotkey_error) = None;
    }
    if settings.autostart != old.autostart {
        let al = app.autolaunch();
        let r = if settings.autostart { al.enable() } else { al.disable() };
        r.map_err(|e| format!("Autostart gagal: {e}"))?;
    }
    let ics_changed = settings.ics_url != old.ics_url;
    {
        let mut s = lk(&st.settings);
        let keep_report = s.last_report_date.clone();
        let keep_plan = s.last_plan_date.clone();
        let keep_chat = s.telegram_chat_id;
        let keep_code = s.telegram_pair_code.clone();
        *s = settings;
        s.first_run_done = true;
        s.last_report_date = keep_report;
        s.last_plan_date = keep_plan;
        s.telegram_chat_id = keep_chat;
        s.telegram_pair_code = keep_code;
        st.store.save("settings.json", &*s);
    }
    if ics_changed {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = automation::refresh_calendar(&app).await {
                lk(&app.state::<Shared>().rt).cal_error = Some(e);
            }
        });
    }
    Ok(())
}

#[tauri::command]
fn set_secret(kind: String, value: String) -> Result<(), String> {
    let entry = secret_entry(&kind).ok_or("Jenis key tidak dikenal")?;
    let value = value.trim();
    if value.is_empty() {
        let _ = entry.delete_credential();
        return Ok(());
    }
    entry.set_password(value).map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_ai(app: AppHandle) -> Result<String, String> {
    let settings = lk(&app.state::<Shared>().settings).clone();
    let key = secret_get(&settings.ai_provider);
    let started = std::time::Instant::now();
    let d = ai::draft(&settings, key, "edit video teaser klien A sebelum jumat").await?;
    Ok(format!(
        "OK dalam {:.1} dtk → \"{}\" · langkah 1: {}",
        started.elapsed().as_secs_f32(),
        d.title,
        d.steps.first().cloned().unwrap_or_default()
    ))
}

#[tauri::command]
async fn local_models(base_url: String, kind: Option<String>) -> Result<Vec<String>, String> {
    ai::list_models(&base_url, secret_get(kind.as_deref().unwrap_or("local"))).await
}

#[tauri::command]
fn hud_action(app: AppHandle, action: String, task_id: Option<String>, value: Option<u32>) {
    let st = app.state::<Shared>();
    match action.as_str() {
        "back" => {
            let hwnd = lk(&st.session).as_mut().map(|s| {
                s.nudging = false;
                s.last_good_hwnd
            });
            if let Some(h) = hwnd.filter(|h| *h != 0) {
                win::focus(h);
            }
            to_pill(&app);
        }
        "allow" => {
            // Pasang niat: boleh di pengganggu N menit, setelah itu ditegur.
            let mins = value.unwrap_or(5).clamp(1, 30) as i64;
            if let Some(s) = lk(&st.session).as_mut() {
                s.allow_until = now_ts() + mins * 60;
                s.allow_label = s.current.clone();
                s.nudging = false;
                s.streak = 0;
            }
            to_pill(&app);
        }
        "snooze" => {
            if let Some(id) = task_id {
                let mut title = None;
                let _ = with_task(&app, &id, |t| {
                    t.snoozes += 1;
                    title = Some(t.title.clone());
                });
                if let Some(t) = title {
                    lk(&st.day).snoozed.push(t);
                }
            }
            to_pill(&app);
        }
        "start" => {
            start_session_inner(&app, task_id, None, None);
        }
        "again" => {
            start_session_inner(&app, task_id, Some(25), None);
        }
        "stop" => finish_session(&app, false),
        "report" => {
            show_main(&app, Some("report"));
            to_pill(&app);
        }
        "open" => {
            show_main(&app, Some("today"));
            to_pill(&app);
        }
        "open_render" => {
            show_main(&app, Some("render"));
            to_pill(&app);
        }
        "render_break" => {
            lk(&st.render).break_taken = true;
            lk(&st.day).breaks += 1;
            to_pill(&app);
        }
        "cancel_power" => {
            lk(&st.rt).cancel_sleep = true;
            let _ = win::command("shutdown").arg("/a").status();
            to_pill(&app);
        }
        _ => {
            if let Some(s) = lk(&st.session).as_mut() {
                s.nudging = false;
            }
            to_pill(&app);
        }
    }
}

#[tauri::command]
fn open_main(app: AppHandle, tab: Option<String>) {
    show_main(&app, tab.as_deref());
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    if win::shell_open(&path) {
        Ok(())
    } else {
        Err("Gagal membuka".into())
    }
}

#[tauri::command]
fn reveal_path(path: String) {
    let _ = win::command("explorer").arg(format!("/select,{path}")).spawn();
}

fn list_monitors(app: AppHandle) -> Vec<String> {
    let Some(w) = app.get_webview_window("hud") else { return vec![] };
    let mut mons = w.available_monitors().unwrap_or_default();
    mons.sort_by_key(|m| (m.position().x, m.position().y));
    mons.iter()
        .enumerate()
        .map(|(i, m)| format!("Monitor {} · {}×{}", i + 1, m.size().width, m.size().height))
        .collect()
}

// --- mode kerja
#[tauri::command]
fn run_preset(app: AppHandle, id: String) -> Result<(), String> {
    automation::run_preset(&app, &id)
}

#[tauri::command]
fn close_distractors(app: AppHandle) -> u32 {
    automation::close_distractors(&app)
}

// --- render
#[tauri::command]
fn render_add(app: AppHandle, file: String, comp: Option<String>, frames: Option<String>, project: Option<String>) -> Result<(), String> {
    render::add(
        &app,
        RenderJob {
            file,
            comp: comp.unwrap_or_default(),
            frames: frames.unwrap_or_default(),
            project: project.filter(|p| !p.trim().is_empty()),
            ..Default::default()
        },
    )
}

#[tauri::command]
fn render_cancel(app: AppHandle, id: String) {
    render::cancel(&app, &id);
}

#[tauri::command]
fn render_remove(app: AppHandle, id: String) {
    render::remove(&app, &id);
}

#[tauri::command]
fn render_retry(app: AppHandle, id: String) {
    render::retry(&app, &id);
}

#[tauri::command]
fn render_clear(app: AppHandle) {
    render::clear_finished(&app);
}

#[tauri::command]
fn render_state(app: AppHandle) -> RenderView {
    render::view(&app)
}

// --- insight
#[tauri::command]
fn get_week(app: AppHandle, end: String) -> insights::Week {
    insights::week(&app, &end)
}

#[tauri::command]
fn get_stats(app: AppHandle) -> insights::Stats {
    insights::stats(&app)
}

#[tauri::command]
fn get_plan(app: AppHandle) -> insights::Plan {
    insights::plan(&app)
}

#[tauri::command]
fn get_clients(app: AppHandle, from: String, to: String) -> Vec<insights::ClientRow> {
    insights::clients(&app, &from, &to)
}

#[tauri::command]
fn export_clients(app: AppHandle, from: String, to: String) -> Result<String, String> {
    insights::export_csv(&app, &from, &to)
}

#[tauri::command]
fn make_invoice(app: AppHandle, project: String, from: String, to: String) -> Result<String, String> {
    insights::invoice(&app, &project, &from, &to)
}

#[tauri::command]
async fn client_update(app: AppHandle, project: String) -> Result<String, String> {
    insights::client_update(&app, &project).await
}

// --- integrasi
#[tauri::command]
fn export_ics(app: AppHandle) -> String {
    automation::export_ics(&app).display().to_string()
}

#[tauri::command]
async fn refresh_calendar(app: AppHandle) -> Result<usize, String> {
    automation::refresh_calendar(&app).await
}

#[tauri::command]
async fn test_webhook(url: String) -> Result<(), String> {
    automation::post_webhook(&url, "👋 Tes dari Gaspol — webhook terhubung.").await
}

#[tauri::command]
fn send_daily_now(app: AppHandle) {
    automation::share_daily(&app, insights::daily_text(&app));
}

#[tauri::command]
fn telegram_pair(app: AppHandle) -> String {
    let code = format!("{:06}", (now_ts().unsigned_abs() * 7919 + std::process::id() as u64) % 1_000_000);
    let st = app.state::<Shared>();
    let mut s = lk(&st.settings);
    s.telegram_pair_code = code.clone();
    s.telegram_chat_id = None;
    st.store.save("settings.json", &*s);
    code
}

#[tauri::command]
fn telegram_unpair(app: AppHandle) {
    let st = app.state::<Shared>();
    let mut s = lk(&st.settings);
    s.telegram_chat_id = None;
    s.telegram_pair_code.clear();
    st.store.save("settings.json", &*s);
}

#[tauri::command]
async fn git_poll_now(app: AppHandle) -> usize {
    let a = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || automation::poll_git(&a)).await;
    lk(&app.state::<Shared>().day).commits.len()
}

// ------------------------------------------------------------------ app

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let capture = MenuItem::with_id(app, "capture", "Tangkap tugas", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Buka Gaspol", true, None::<&str>)?;
    let render = MenuItem::with_id(app, "render", "Antrian render", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Hentikan sesi fokus", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&capture, &open, &render, &stop, &sep, &quit])?;
    let mut builder = TrayIconBuilder::with_id("gaspol")
        .tooltip("Gaspol")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "capture" => toggle_capture(app),
            "open" => show_main(app, None),
            "render" => show_main(app, Some("render")),
            "stop" => finish_session(app, false),
            "quit" => {
                finish_session(app, false);
                let st = app.state::<Shared>();
                let d = lk(&st.day).clone();
                st.store.save(&Store::day_name(&d.date), &d);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle(), None);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn on_hotkey(app: &AppHandle, shortcut: &Shortcut) {
    let sel = lk(&app.state::<Shared>().settings).selection_hotkey.clone();
    let is_sel = sel.parse::<Shortcut>().map(|s| &s == shortcut).unwrap_or(false);
    if !is_sel {
        toggle_capture(app);
        return;
    }
    // Ambil teks yang sedang diblok di app lain, lalu buka capture bar dengan isi itu.
    let app = app.clone();
    std::thread::spawn(move || {
        let text = win::copy_selection();
        open_capture(&app, text);
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app, None)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        on_hotkey(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(Shared::load(dir));
            let handle = app.handle().clone();
            build_tray(&handle)?;

            let st = handle.state::<Shared>();
            let (hotkey, sel, first_run, autostart) = {
                let s = lk(&st.settings);
                (s.hotkey.clone(), s.selection_hotkey.clone(), !s.first_run_done, s.autostart)
            };
            if let Err(e) = register_hotkeys(&handle, &hotkey, &sel) {
                *lk(&st.hotkey_error) = Some(e);
            }
            // Daftar ulang tiap start supaya path-nya selalu ke exe yang sedang dipakai.
            if autostart {
                let _ = handle.autolaunch().enable();
            }
            if first_run {
                let mut s = lk(&st.settings);
                s.first_run_done = true;
                st.store.save("settings.json", &*s);
            }
            if !std::env::args().any(|a| a == "--hidden") {
                show_main(&handle, None);
            }
            for label in ["capture", "hud"] {
                if let Some(w) = handle.get_webview_window(label) {
                    transparent(&w);
                }
            }
            render::load(&handle);
            render::spawn_worker(handle.clone());
            render::spawn_watcher(handle.clone());
            automation::spawn_sync(handle.clone());
            automation::spawn_telegram(handle.clone());
            tracker::spawn(handle.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Jendela utama boleh benar-benar tertutup; capture & HUD cuma disembunyikan.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            capture_task,
            capture_image,
            transcribe,
            update_task,
            delete_task,
            toggle_step,
            complete_task,
            regenerate_steps,
            start_session,
            stop_session,
            get_day,
            save_settings,
            set_secret,
            test_ai,
            local_models,
            hud_action,
            open_main,
            open_path,
            reveal_path,
            run_preset,
            close_distractors,
            render_add,
            render_cancel,
            render_remove,
            render_retry,
            render_clear,
            render_state,
            get_week,
            get_stats,
            get_plan,
            get_clients,
            export_clients,
            make_invoice,
            client_update,
            export_ics,
            refresh_calendar,
            test_webhook,
            send_daily_now,
            telegram_pair,
            telegram_unpair,
            git_poll_now
        ])
        .build(tauri::generate_context!())
        .expect("gagal menjalankan Gaspol")
        .run(|_app, event| {
            // Tutup dashboard ≠ keluar app; keluar hanya lewat menu tray (exit code eksplisit).
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
