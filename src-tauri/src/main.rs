#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod store;
mod tracker;
mod win;

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
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracker::{Session, SessionView};

pub struct Shared {
    pub store: Store,
    pub tasks: Mutex<Vec<Task>>,
    pub settings: Mutex<Settings>,
    pub session: Mutex<Option<Session>>,
    pub day: Mutex<DayLog>,
    pub hotkey_error: Mutex<Option<String>>,
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

fn secret_entry(kind: &str) -> Option<keyring::Entry> {
    matches!(kind, "claude" | "local" | "custom").then(|| keyring::Entry::new("gaspol", kind).ok())?
}

fn secret_get(kind: &str) -> Option<String> {
    secret_entry(kind)?.get_password().ok().filter(|s| !s.is_empty())
}

fn emit_tasks(app: &AppHandle) {
    let st = app.state::<Shared>();
    let tasks = lk(&st.tasks).clone();
    let _ = app.emit("tasks", tasks);
}

fn place(w: &WebviewWindow, top_ratio: f64, top_px: f64) {
    let Ok(Some(m)) = w.primary_monitor() else { return };
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
        let top = lk(&app.state::<Shared>().settings).hud_top.min(400);
        place(&w, 0.0, top as f64);
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

/// Kembalikan HUD ke pil timer (kalau ada sesi), atau sembunyikan.
pub fn to_pill(app: &AppHandle) {
    let active = lk(&app.state::<Shared>().session).is_some();
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

fn toggle_capture(app: &AppHandle) {
    let Some(w) = app.get_webview_window("capture") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    } else {
        place(&w, 0.2, 0.0);
        let _ = w.show();
        transparent(&w);
        let _ = w.set_focus();
        let _ = app.emit("capture-open", ());
    }
}

fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(hotkey).map_err(|e| format!("Hotkey \"{hotkey}\" gagal dipasang: {e}"))
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
            }),
            true,
        );
    } else {
        hud_hide(app);
    }
}

fn start_session_inner(app: &AppHandle, task_id: Option<String>, minutes: Option<u32>, mode: Option<String>) -> SessionView {
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
    let (task_id, title, step) = {
        let mut tasks = lk(&st.tasks);
        let found = task_id.as_ref().and_then(|id| tasks.iter_mut().find(|t| &t.id == id));
        let out = match found {
            Some(t) => {
                t.last_touched = Some(now);
                (Some(t.id.clone()), t.title.clone(), t.steps.iter().find(|s| !s.done).map(|s| s.text.clone()))
            }
            None => (None, "Sesi fokus bebas".to_string(), None),
        };
        st.store.save("tasks.json", &*tasks);
        out
    };
    let s = Session {
        task_id,
        task_title: title,
        step,
        start: now,
        planned_secs: minutes as u64 * 60,
        mode,
        focus_secs: 0,
        distracted_secs: 0,
        distractions: 0,
        streak: 0,
        last_good_hwnd: 0,
        nudging: false,
        current: String::new(),
        distracted: false,
    };
    let view = s.view(now);
    *lk(&st.session) = Some(s);
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
    hotkey_error: Option<String>,
    data_dir: String,
    days: Vec<String>,
}

#[tauri::command]
fn get_state(st: State<'_, Shared>) -> StateView {
    let session = lk(&st.session).as_ref().map(|s| s.view(now_ts())).unwrap_or_default();
    StateView {
        tasks: lk(&st.tasks).clone(),
        settings: lk(&st.settings).clone(),
        session,
        has_claude_key: secret_get("claude").is_some(),
        has_local_key: secret_get("local").is_some(),
        has_custom_key: secret_get("custom").is_some(),
        hotkey_error: lk(&st.hotkey_error).clone(),
        data_dir: st.store.dir.display().to_string(),
        days: st.store.list_days(),
    }
}

#[derive(Serialize)]
struct CaptureResult {
    task: Task,
    warning: Option<String>,
}

#[tauri::command]
async fn capture_task(app: AppHandle, text: String) -> Result<CaptureResult, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Tulis dulu tugasnya".into());
    }
    let settings = lk(&app.state::<Shared>().settings).clone();
    let (draft, ai_used, warning) = if settings.ai_provider == "none" {
        (ai::heuristic(&text), false, None)
    } else {
        let key = secret_get(&settings.ai_provider);
        match ai::draft(&settings, key, &text).await {
            Ok(d) => (d, true, None),
            Err(e) => (ai::heuristic(&text), false, Some(e)),
        }
    };
    let task = Task {
        id: new_id(),
        title: draft.title,
        project: draft.project,
        deadline: draft.deadline,
        est_minutes: draft.est_minutes,
        steps: draft.steps.into_iter().map(|text| Step { text, done: false }).collect(),
        created_at: now_ts(),
        raw: text,
        ai: ai_used,
        ..Default::default()
    };
    {
        let st = app.state::<Shared>();
        let mut tasks = lk(&st.tasks);
        tasks.push(task.clone());
        st.store.save("tasks.json", &*tasks);
    }
    emit_tasks(&app);
    Ok(CaptureResult { task, warning })
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
        t.project = task.project;
        t.deadline = task.deadline.filter(|d| !d.is_empty());
        t.steps = task.steps;
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
    with_task(&app, &id, |t| {
        t.done = done;
        t.done_at = done.then(now_ts);
    })
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
    if settings.hotkey != old.hotkey {
        if let Err(e) = register_hotkey(&app, &settings.hotkey) {
            let _ = register_hotkey(&app, &old.hotkey);
            return Err(e);
        }
        *lk(&st.hotkey_error) = None;
    }
    if settings.autostart != old.autostart {
        let al = app.autolaunch();
        let r = if settings.autostart { al.enable() } else { al.disable() };
        r.map_err(|e| format!("Autostart gagal: {e}"))?;
    }
    let mut s = lk(&st.settings);
    let keep_report = s.last_report_date.clone();
    *s = settings;
    s.first_run_done = true;
    s.last_report_date = keep_report;
    st.store.save("settings.json", &*s);
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
fn hud_action(app: AppHandle, action: String, task_id: Option<String>) {
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

// ------------------------------------------------------------------ app

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let capture = MenuItem::with_id(app, "capture", "Tangkap tugas", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Buka Gaspol", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Hentikan sesi fokus", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&capture, &open, &stop, &sep, &quit])?;
    let mut builder = TrayIconBuilder::with_id("gaspol")
        .tooltip("Gaspol")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "capture" => toggle_capture(app),
            "open" => show_main(app, None),
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app, None)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
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
            let (hotkey, first_run, autostart) = {
                let s = lk(&st.settings);
                (s.hotkey.clone(), !s.first_run_done, s.autostart)
            };
            if let Err(e) = register_hotkey(&handle, &hotkey) {
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
            open_main
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
