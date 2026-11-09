//! Antrian render Blender / After Effects di background, deteksi render yang
//! sedang jalan di luar Gaspol, dan perapian otomatis file hasil export.

use crate::{hud_message, lk, now_ts, today_str, win, Shared};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct RenderJob {
    pub id: String,
    /// "blender" | "ae"
    pub kind: String,
    pub file: String,
    /// After Effects: nama comp (kosong = semua item di Render Queue).
    pub comp: String,
    /// Blender: "anim" (default) atau nomor frame tunggal.
    pub frames: String,
    pub project: Option<String>,
    /// queued | running | done | failed | canceled
    pub status: String,
    pub progress: f32,
    pub frame: Option<i64>,
    pub total: Option<i64>,
    pub started: Option<i64>,
    pub ended: Option<i64>,
    pub message: String,
    pub output: Option<String>,
}

#[derive(Default)]
pub struct RenderState {
    pub jobs: Vec<RenderJob>,
    pub pid: Option<u32>,
    pub cancel: bool,
    /// Render di luar Gaspol yang terdeteksi: (label, sejak).
    pub external: Option<(String, i64)>,
    pub offered: bool,
    pub break_taken: bool,
    pub seen: HashMap<PathBuf, (u64, u8)>,
    pub started_at: i64,
}

#[derive(Serialize, Clone)]
pub struct RenderView {
    pub jobs: Vec<RenderJob>,
    pub external: Option<String>,
    pub external_secs: i64,
    pub blender: Option<String>,
    pub aerender: Option<String>,
}

pub fn view(app: &AppHandle) -> RenderView {
    let st = app.state::<Shared>();
    let s = lk(&st.settings).clone();
    let r = lk(&st.render);
    RenderView {
        jobs: r.jobs.clone(),
        external: r.external.as_ref().map(|e| e.0.clone()),
        external_secs: r.external.as_ref().map(|e| now_ts() - e.1).unwrap_or(0),
        blender: exe_or_find(&s.blender_path, find_blender),
        aerender: exe_or_find(&s.aerender_path, find_aerender),
    }
}

pub fn emit(app: &AppHandle) {
    let _ = app.emit("render", view(app));
}

fn save(app: &AppHandle) {
    let st = app.state::<Shared>();
    let jobs = lk(&st.render).jobs.clone();
    st.store.save("render_queue.json", &jobs);
}

pub fn load(app: &AppHandle) {
    let st = app.state::<Shared>();
    let mut jobs: Vec<RenderJob> = st.store.load("render_queue.json");
    // Job yang terputus karena app ditutup → antrikan ulang.
    for j in jobs.iter_mut().filter(|j| j.status == "running") {
        j.status = "queued".into();
        j.progress = 0.0;
    }
    let mut r = lk(&st.render);
    r.jobs = jobs;
    r.started_at = now_ts();
}

fn exe_or_find(path: &str, find: fn() -> Option<String>) -> Option<String> {
    let p = path.trim();
    if !p.is_empty() && Path::new(p).exists() {
        return Some(p.to_string());
    }
    find()
}

fn newest_match(root: &str, dir_prefix: &str, rel: &str) -> Option<String> {
    // Urut berdasarkan angka versi di nama folder ("2026" > "CC 2019", "5.1" > "4.5").
    let mut found: Vec<(Vec<u32>, PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(dir_prefix))
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let ver: Vec<u32> = name
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|p| p.parse().ok())
                .collect();
            (ver, e.path().join(rel))
        })
        .filter(|(_, p)| p.exists())
        .collect();
    found.sort();
    found.pop().map(|(_, p)| p.display().to_string())
}

pub fn find_blender() -> Option<String> {
    newest_match(r"C:\Program Files\Blender Foundation", "Blender", "blender.exe").or_else(|| {
        let steam = r"C:\Program Files (x86)\Steam\steamapps\common\Blender\blender.exe";
        Path::new(steam).exists().then(|| steam.to_string())
    })
}

pub fn find_aerender() -> Option<String> {
    newest_match(r"C:\Program Files\Adobe", "Adobe After Effects", r"Support Files\aerender.exe")
}

pub fn add(app: &AppHandle, mut job: RenderJob) -> Result<(), String> {
    if !Path::new(&job.file).exists() {
        return Err("File tidak ditemukan".into());
    }
    let ext = Path::new(&job.file)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    job.kind = match ext.as_str() {
        "blend" => "blender".into(),
        "aep" | "aepx" => "ae".into(),
        _ => return Err("Cuma bisa .blend atau .aep".into()),
    };
    job.id = format!("r{:x}", chrono::Local::now().timestamp_millis());
    job.status = "queued".into();
    job.progress = 0.0;
    lk(&app.state::<Shared>().render).jobs.push(job);
    save(app);
    emit(app);
    Ok(())
}

pub fn cancel(app: &AppHandle, id: &str) {
    let st = app.state::<Shared>();
    let pid = {
        let mut r = lk(&st.render);
        let mut pid = None;
        let running_pid = r.pid;
        if let Some(j) = r.jobs.iter_mut().find(|j| j.id == id) {
            if j.status == "running" {
                pid = running_pid;
            } else if j.status == "queued" {
                j.status = "canceled".into();
            }
        }
        if pid.is_some() {
            r.cancel = true;
        }
        pid
    };
    if let Some(pid) = pid {
        let _ = win::command("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).status();
    }
    save(app);
    emit(app);
}

pub fn remove(app: &AppHandle, id: &str) {
    lk(&app.state::<Shared>().render).jobs.retain(|j| j.id != id || j.status == "running");
    save(app);
    emit(app);
}

pub fn clear_finished(app: &AppHandle) {
    lk(&app.state::<Shared>().render)
        .jobs
        .retain(|j| j.status == "queued" || j.status == "running");
    save(app);
    emit(app);
}

pub fn retry(app: &AppHandle, id: &str) {
    if let Some(j) = lk(&app.state::<Shared>().render).jobs.iter_mut().find(|j| j.id == id) {
        if j.status != "running" {
            j.status = "queued".into();
            j.progress = 0.0;
            j.message.clear();
        }
    }
    save(app);
    emit(app);
}

/// Worker: satu job pada satu waktu, berurutan.
pub fn spawn_worker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        let next = {
            let st = app.state::<Shared>();
            let mut r = lk(&st.render);
            if r.pid.is_some() {
                continue;
            }
            r.jobs.iter_mut().find(|j| j.status == "queued").map(|j| {
                j.status = "running".into();
                j.started = Some(now_ts());
                j.clone()
            })
        };
        let Some(job) = next else { continue };
        emit(&app);
        crate::to_pill(&app);
        let result = run(&app, &job);
        let st = app.state::<Shared>();
        let (finished, queue_empty, canceled) = {
            let mut r = lk(&st.render);
            let canceled = std::mem::take(&mut r.cancel);
            r.pid = None;
            let empty = !r.jobs.iter().any(|j| j.status == "queued");
            let j = r.jobs.iter_mut().find(|j| j.id == job.id);
            let mut out = None;
            if let Some(j) = j {
                j.ended = Some(now_ts());
                match &result {
                    _ if canceled => {
                        j.status = "canceled".into();
                        j.message = "Dibatalkan".into();
                    }
                    Ok(()) => {
                        j.status = "done".into();
                        j.progress = 1.0;
                    }
                    Err(e) => {
                        j.status = "failed".into();
                        j.message = e.clone();
                    }
                }
                out = Some(j.clone());
            }
            (out, empty, canceled)
        };
        save(&app);
        emit(&app);
        let Some(done) = finished else { continue };
        if canceled {
            continue;
        }
        if done.status == "done" {
            lk(&st.day).renders += 1;
            let organize = lk(&st.settings).organize_exports;
            if let (true, Some(out)) = (organize, done.output.as_deref()) {
                organize_file(&app, Path::new(out), done.project.clone());
            }
        }
        let name = Path::new(&done.file)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        hud_message(
            &app,
            json!({
                "kind": "render_done",
                "ok": done.status == "done",
                "name": name,
                "secs": done.ended.unwrap_or(0) - done.started.unwrap_or(0),
                "message": done.message,
                "output": done.output,
            }),
            true,
        );
        crate::notify_remote(&app, &format!(
            "{} Render {}: {}",
            if done.status == "done" { "✅" } else { "❌" },
            if done.status == "done" { "selesai" } else { "gagal" },
            name
        ));
        if queue_empty {
            after_queue(&app);
        }
    });
}

fn after_queue(app: &AppHandle) {
    let action = lk(&app.state::<Shared>().settings).after_queue.clone();
    match action.as_str() {
        "shutdown" => {
            let _ = win::command("shutdown").args(["/s", "/t", "120", "/c", "Gaspol: antrian render selesai"]).status();
            hud_message(app, json!({ "kind": "power", "action": "shutdown" }), true);
        }
        "sleep" => {
            hud_message(app, json!({ "kind": "power", "action": "sleep" }), true);
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(60));
                let st = app.state::<Shared>();
                if lk(&st.rt).cancel_sleep {
                    lk(&st.rt).cancel_sleep = false;
                    return;
                }
                let _ = win::command("rundll32.exe").args(["powrprof.dll,SetSuspendState", "0,1,0"]).status();
            });
        }
        _ => {}
    }
}

fn update<F: FnOnce(&mut RenderJob)>(app: &AppHandle, id: &str, f: F) {
    if let Some(j) = lk(&app.state::<Shared>().render).jobs.iter_mut().find(|j| j.id == id) {
        f(j);
    }
}

const PY_RANGE: &str = "import bpy;s=bpy.context.scene;print('GASPOL_RANGE',s.frame_start,s.frame_end,bpy.path.abspath(s.render.filepath),flush=True)";

fn run(app: &AppHandle, job: &RenderJob) -> Result<(), String> {
    let s = lk(&app.state::<Shared>().settings).clone();
    let mut cmd = if job.kind == "blender" {
        let exe = exe_or_find(&s.blender_path, find_blender).ok_or("blender.exe tidak ditemukan — isi path di Pengaturan")?;
        let mut c = win::command(&exe);
        c.args(["-b", &job.file, "--python-expr", PY_RANGE]);
        match job.frames.trim() {
            "" | "anim" => {
                c.arg("-a");
            }
            f => {
                c.args(["-f", f]);
            }
        }
        c
    } else {
        let exe = exe_or_find(&s.aerender_path, find_aerender).ok_or("aerender.exe tidak ditemukan — isi path di Pengaturan")?;
        let mut c = win::command(&exe);
        c.args(["-project", &job.file]);
        if !job.comp.trim().is_empty() {
            c.args(["-comp", job.comp.trim()]);
        }
        c
    };
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("gagal menjalankan renderer: {e}"))?;
    lk(&app.state::<Shared>().render).pid = Some(child.id());

    let stdout = child.stdout.take().ok_or("stdout tidak tersedia")?;
    let (mut start, mut total, mut fps) = (0i64, 0i64, 0f64);
    let mut dur = String::new();
    let mut last_line = String::new();
    let mut last_emit = std::time::Instant::now();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let mut l = line.trim();
        // Blender 5: "00:02.328  render           | Saved: '...'"
        if job.kind == "blender" {
            if let Some((_, rest)) = l.split_once(" | ") {
                l = rest.trim();
            }
        }
        if l.is_empty() {
            continue;
        }
        last_line = l.chars().take(200).collect();
        if job.kind == "blender" {
            if let Some(rest) = l.strip_prefix("GASPOL_RANGE ") {
                let parts: Vec<&str> = rest.splitn(3, ' ').collect();
                start = parts.first().and_then(|v| v.parse().ok()).unwrap_or(1);
                let end: i64 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(start);
                total = if job.frames.trim().is_empty() || job.frames == "anim" { end - start + 1 } else { 1 };
                let out = parts.get(2).map(|s| s.to_string());
                update(app, &job.id, |j| {
                    j.total = Some(total);
                    if j.output.is_none() {
                        j.output = out.filter(|o| !o.is_empty());
                    }
                });
            } else if let Some(rest) = l.strip_prefix("Fra:").or_else(|| l.strip_prefix("Rendering frame ")) {
                let f: i64 = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(start);
                let p = if total > 0 { ((f - start) as f32 / total as f32).clamp(0.0, 0.999) } else { 0.0 };
                update(app, &job.id, |j| {
                    j.frame = Some(f);
                    j.progress = j.progress.max(p);
                });
            } else if let Some(rest) = l.strip_prefix("Saved: ") {
                let path = rest.trim_matches(|c| c == '\'' || c == '"').to_string();
                update(app, &job.id, |j| {
                    j.output = Some(path);
                    if total > 0 {
                        let saved = j.frame.map(|f| f - start + 1).unwrap_or(1);
                        j.progress = j.progress.max((saved as f32 / total as f32).min(0.999));
                    }
                });
            }
        } else if let Some(rest) = l.split("PROGRESS:").nth(1).map(str::trim) {
            if let Some(d) = rest.strip_prefix("Duration:") {
                dur = d.trim().to_string();
                total = timecode_frames(&dur, if fps > 0.0 { fps } else { 30.0 });
            } else if let Some(r) = rest.strip_prefix("Frame Rate:") {
                fps = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(30.0);
                if !dur.is_empty() {
                    total = timecode_frames(&dur, fps);
                }
            } else if let (Some(a), Some(b)) = (rest.find('('), rest.find(')')) {
                if let Ok(f) = rest[a + 1..b].trim().parse::<i64>() {
                    let p = if total > 0 { (f as f32 / total as f32).clamp(0.0, 0.999) } else { 0.0 };
                    update(app, &job.id, |j| {
                        j.frame = Some(f);
                        j.total = (total > 0).then_some(total);
                        j.progress = j.progress.max(p);
                    });
                }
            } else if let Some(o) = rest.strip_prefix("Output To:") {
                let o = o.trim().to_string();
                update(app, &job.id, |j| j.output = Some(o));
            }
        }
        if last_emit.elapsed().as_millis() > 700 {
            last_emit = std::time::Instant::now();
            emit(app);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(if last_line.is_empty() { format!("keluar dengan kode {:?}", status.code()) } else { last_line })
    }
}

/// `0;00;15;00` atau `0:00:15:00` → jumlah frame.
fn timecode_frames(tc: &str, fps: f64) -> i64 {
    let n: Vec<i64> = tc
        .split([';', ':'])
        .filter_map(|p| p.trim().split_whitespace().next()?.parse().ok())
        .collect();
    if n.len() < 4 {
        return 0;
    }
    ((n[0] * 3600 + n[1] * 60 + n[2]) as f64 * fps) as i64 + n[3]
}

// ------------------------------------------------------------------ render di luar Gaspol

/// Dipanggil tracker tiap 5 detik.
pub fn tick_external(app: &AppHandle) {
    let st = app.state::<Shared>();
    let ours = lk(&st.render).pid.is_some();
    let procs = win::processes();
    let label = if ours {
        Some("antrian Gaspol".to_string())
    } else if procs.iter().any(|p| p == "aerender") {
        Some("After Effects".to_string())
    } else if procs.iter().any(|p| p == "ffmpeg") {
        Some("ffmpeg".to_string())
    } else if win::list_windows()
        .iter()
        .any(|w| w.exe == "blender" && w.title.to_lowercase().contains("blender render"))
    {
        Some("Blender".to_string())
    } else {
        None
    };
    let now = now_ts();
    let (offer, done) = {
        let mut r = lk(&st.render);
        match (&label, r.external.clone()) {
            (Some(l), None) => {
                r.external = Some((l.clone(), now));
                r.offered = false;
                r.break_taken = false;
                (None, None)
            }
            (Some(_), Some((l, since))) => {
                let long = now - since >= 300;
                if long && !r.offered {
                    r.offered = true;
                    (Some((l, now - since)), None)
                } else {
                    (None, None)
                }
            }
            (None, Some((l, since))) => {
                r.external = None;
                let took = r.break_taken;
                r.break_taken = false;
                // Job antrian sudah punya notifikasi sendiri.
                let notify = l != "antrian Gaspol" && (took || now - since >= 120);
                (None, notify.then_some((l, now - since)))
            }
            (None, None) => (None, None),
        }
    };
    let render_break = lk(&st.settings).render_break;
    if let Some((l, secs)) = offer.filter(|_| render_break) {
        hud_message(app, json!({ "kind": "render", "label": l, "secs": secs }), true);
    }
    if let Some((l, secs)) = done {
        hud_message(app, json!({ "kind": "render_done", "ok": true, "name": l, "secs": secs, "external": true }), true);
        crate::notify_remote(app, &format!("✅ Render {l} selesai"));
    }
}

/// Lagi istirahat menunggu render → pengganggu tidak dihitung.
pub fn on_break(app: &AppHandle) -> bool {
    let st = app.state::<Shared>();
    let r = lk(&st.render);
    r.break_taken && r.external.is_some()
}

// ------------------------------------------------------------------ rapikan export

const MEDIA: &[&str] = &["mp4", "mov", "webm", "mkv", "avi", "gif", "wav", "mp3", "m4a", "prores", "mxf"];

/// Pantau folder export: file media baru yang ukurannya sudah stabil dipindah & diberi nama rapi.
pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(10));
        let s = lk(&app.state::<Shared>().settings).clone();
        if !s.organize_exports || s.watch_dirs.is_empty() {
            continue;
        }
        let started = lk(&app.state::<Shared>().render).started_at;
        let mut ready = Vec::new();
        for dir in s.watch_dirs.iter().map(|d| d.trim()).filter(|d| !d.is_empty()) {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                if !MEDIA.contains(&ext.as_str()) {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                if !meta.is_file() || modified < started {
                    continue;
                }
                let size = meta.len();
                let st = app.state::<Shared>();
                let mut r = lk(&st.render);
                let entry = r.seen.entry(p.clone()).or_insert((size, 0));
                if entry.1 == 255 {
                    continue;
                }
                if entry.0 == size && size > 0 {
                    entry.1 += 1;
                } else {
                    *entry = (size, 0);
                }
                if entry.1 >= 2 {
                    entry.1 = 255;
                    ready.push(p);
                }
            }
        }
        for p in ready {
            let project = crate::current_project(&app);
            organize_file(&app, &p, project);
        }
    });
}

fn slug(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    out.split('_').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("_")
}

/// `<root>/<project>/<tanggal>_<nama>_v01.mp4`
pub fn organize_file(app: &AppHandle, path: &Path, project: Option<String>) {
    let s = lk(&app.state::<Shared>().settings).clone();
    if !path.is_file() {
        return;
    }
    let root = if s.export_root.trim().is_empty() {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        PathBuf::from(s.export_root.trim())
    };
    let folder = root.join(slug(project.as_deref().unwrap_or("Umum")));
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    // Buang akhiran versi lama biar penomoran konsisten.
    let base = {
        let st = slug(&stem);
        match st.rfind("_v") {
            Some(i) if st[i + 2..].chars().all(|c| c.is_ascii_digit()) && i + 2 < st.len() => st[..i].to_string(),
            _ => st,
        }
    };
    let ext = path.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    let date = today_str();
    let mut v = 1;
    let target = loop {
        let name = format!("{date}_{base}_v{v:02}.{ext}");
        let t = folder.join(&name);
        let taken = std::fs::read_dir(&folder)
            .map(|rd| {
                rd.filter_map(|e| e.ok()).any(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.contains(&format!("_{base}_v{v:02}."))
                })
            })
            .unwrap_or(false);
        if !taken && !t.exists() {
            break t;
        }
        v += 1;
        if v > 999 {
            return;
        }
    };
    if path == target {
        return;
    }
    let moved = std::fs::rename(path, &target).is_ok()
        || (std::fs::copy(path, &target).is_ok() && std::fs::remove_file(path).is_ok());
    if moved {
        let _ = app.emit(
            "toast",
            format!("Export dirapikan → {}", target.display()),
        );
    }
}
