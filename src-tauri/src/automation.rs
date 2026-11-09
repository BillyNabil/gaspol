//! Mode Kerja, musik fokus, blokir situs, notifikasi keluar (Discord/Telegram),
//! kalender ICS, dan Git.

use crate::store::{CommitLog, Store};
use crate::{lk, now_ts, secret_get, tracker, win, Shared};
use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone};
use serde::Serialize;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// State runtime yang tidak perlu disimpan ke disk.
#[derive(Default)]
pub struct Runtime {
    pub active_streak: u32,
    pub idle_run: u32,
    pub blocked: bool,
    pub music_started: bool,
    pub cancel_sleep: bool,
    pub cal: Vec<CalEvent>,
    pub cal_error: Option<String>,
    pub last_cal: i64,
    pub last_git: i64,
    pub auto_task: Option<(String, u32)>,
    pub telegram_ok: bool,
    pub telegram_error: Option<String>,
}

// ------------------------------------------------------------------ mode kerja

pub fn run_preset(app: &AppHandle, id: &str) -> Result<(), String> {
    let st = app.state::<Shared>();
    let p = lk(&st.settings)
        .presets
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or("Preset tidak ditemukan")?;
    let mut failed = Vec::new();
    for target in p.open.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if !win::shell_open(target) {
            failed.push(target.to_string());
        }
    }
    if p.close_distractors {
        close_distractors(app);
    }
    // Tugas: yang paling mendesak di project preset (kalau ada).
    let task_id = {
        let tasks = lk(&st.tasks);
        let mut open: Vec<_> = tasks
            .iter()
            .filter(|t| !t.done)
            .filter(|t| match &p.project {
                Some(pr) if !pr.trim().is_empty() => t.project.as_deref().map(|x| x.eq_ignore_ascii_case(pr.trim())).unwrap_or(false),
                _ => false,
            })
            .collect();
        open.sort_by_key(|t| t.deadline.clone().unwrap_or_else(|| "9999".into()));
        open.first().map(|t| t.id.clone())
    };
    let has_task = task_id.is_some();
    crate::start_session_inner(app, task_id, Some(p.minutes.max(5)), Some(p.mode.clone()));
    if !has_task {
        if let Some(s) = lk(&st.session).as_mut() {
            s.task_title = p.name.clone();
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("Gagal dibuka: {}", failed.join(", ")))
    }
}

/// Tutup app pengganggu (Discord, Steam, ...). Jendela browser cuma di-minimize biar tab tidak hilang.
pub fn close_distractors(app: &AppHandle) -> u32 {
    let st = app.state::<Shared>();
    let (distractors, allow) = {
        let s = lk(&st.settings);
        (s.distractors.clone(), s.allow_keywords.clone())
    };
    let own = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mut n = 0;
    for w in win::list_windows() {
        if w.exe == own || w.exe.is_empty() {
            continue;
        }
        let c = tracker::classify(&w.exe, &w.title, &distractors, &allow, &[]);
        if !c.distracting {
            continue;
        }
        if tracker::is_browser(&w.exe) {
            win::minimize(w.hwnd);
        } else {
            win::close_window(w.hwnd);
        }
        n += 1;
    }
    n
}

pub fn on_session_start(app: &AppHandle, mode: &str) {
    let st = app.state::<Shared>();
    let (music, block) = {
        let s = lk(&st.settings);
        (s.music.trim().to_string(), s.hard_block && mode == "strict")
    };
    if !music.is_empty() {
        if music == "media" {
            win::media_play_pause();
        } else {
            win::shell_open(&music);
        }
        lk(&st.rt).music_started = true;
    }
    if block {
        set_block(app, true);
    }
}

pub fn on_session_end(app: &AppHandle) {
    let st = app.state::<Shared>();
    let (music_started, blocked) = {
        let mut rt = lk(&st.rt);
        (std::mem::take(&mut rt.music_started), rt.blocked)
    };
    if music_started && lk(&st.settings).music.trim() == "media" {
        win::media_play_pause();
    }
    if blocked {
        set_block(app, false);
    }
}

/// Tulis/hapus blok `# gaspol` di file hosts lewat PowerShell ber-hak admin (UAC).
pub fn set_block(app: &AppHandle, on: bool) {
    let st = app.state::<Shared>();
    let sites: Vec<String> = lk(&st.settings)
        .block_sites
        .iter()
        .map(|s| s.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .collect();
    let lines: String = sites.iter().map(|s| format!("0.0.0.0 {s}`r`n")).collect();
    let script = format!(
        r#"$h = "$env:SystemRoot\System32\drivers\etc\hosts"
$t = [IO.File]::ReadAllText($h)
$t = [regex]::Replace($t, "(?s)\r?\n# gaspol-start.*?# gaspol-end\r?\n?", "")
if ('{mode}' -eq 'on') {{ $t = $t.TrimEnd() + "`r`n# gaspol-start`r`n{lines}# gaspol-end`r`n" }}
[IO.File]::WriteAllText($h, $t)
ipconfig /flushdns | Out-Null
"#,
        mode = if on { "on" } else { "off" },
    );
    let path = st.store.dir.join("hosts-block.ps1");
    if std::fs::write(&path, script).is_err() {
        return;
    }
    let params = format!("-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"", path.display());
    if win::shell_runas("powershell.exe", &params) {
        lk(&st.rt).blocked = on;
    }
}

// ------------------------------------------------------------------ notifikasi keluar

/// Kirim ke Telegram (kalau terhubung). Dipakai untuk render selesai, pengingat, laporan.
pub fn notify_remote(app: &AppHandle, text: &str) {
    let chat = lk(&app.state::<Shared>().settings).telegram_chat_id;
    let (Some(chat), Some(token)) = (chat, secret_get("telegram")) else { return };
    let text = text.to_string();
    tauri::async_runtime::spawn(async move {
        let _ = tg_send(&token, chat, &text).await;
    });
}

/// Ringkasan harian ke webhook Discord (akuntabilitas) + Telegram.
pub fn share_daily(app: &AppHandle, text: String) {
    let url = lk(&app.state::<Shared>().settings).webhook_url.trim().to_string();
    notify_remote(app, &text);
    if url.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = post_webhook(&url, &text).await;
    });
}

pub async fn post_webhook(url: &str, text: &str) -> Result<(), String> {
    let body = if url.contains("hooks.slack.com") { json!({ "text": text }) } else { json!({ "content": text, "username": "Gaspol" }) };
    let res = client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", res.status()))
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(70))
        .build()
        .unwrap_or_default()
}

// ------------------------------------------------------------------ Telegram

async fn tg_send(token: &str, chat: i64, text: &str) -> Result<(), String> {
    client()
        .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
        .json(&json!({ "chat_id": chat, "text": text }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Bot Telegram: kirim kalimat apa pun → jadi tugas. `/hari`, `/fokus`, `/stop`, `/laporan`.
pub fn spawn_telegram(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut offset: i64 = 0;
        loop {
            let Some(token) = secret_get("telegram") else {
                lk(&app.state::<Shared>().rt).telegram_ok = false;
                tokio_sleep(15).await;
                continue;
            };
            let url = format!("https://api.telegram.org/bot{token}/getUpdates?timeout=50&offset={offset}");
            let res = match client().get(&url).send().await {
                Ok(r) => r.json::<serde_json::Value>().await.ok(),
                Err(e) => {
                    lk(&app.state::<Shared>().rt).telegram_error = Some(e.to_string());
                    None
                }
            };
            let Some(v) = res else {
                tokio_sleep(10).await;
                continue;
            };
            if v["ok"].as_bool() != Some(true) {
                {
                    let st = app.state::<Shared>();
                    let mut rt = lk(&st.rt);
                    rt.telegram_ok = false;
                    rt.telegram_error = v["description"].as_str().map(String::from);
                }
                tokio_sleep(30).await;
                continue;
            }
            {
                let st = app.state::<Shared>();
                let mut rt = lk(&st.rt);
                rt.telegram_ok = true;
                rt.telegram_error = None;
            }
            for u in v["result"].as_array().cloned().unwrap_or_default() {
                offset = offset.max(u["update_id"].as_i64().unwrap_or(0) + 1);
                let Some(chat) = u["message"]["chat"]["id"].as_i64() else { continue };
                let text = u["message"]["text"].as_str().unwrap_or("").trim().to_string();
                if text.is_empty() {
                    continue;
                }
                let reply = handle_telegram(&app, chat, &text).await;
                if let Some(r) = reply {
                    let _ = tg_send(&token, chat, &r).await;
                }
            }
        }
    });
}

async fn tokio_sleep(secs: u64) {
    tokio::time::sleep(Duration::from_secs(secs)).await;
}

async fn handle_telegram(app: &AppHandle, chat: i64, text: &str) -> Option<String> {
    let st = app.state::<Shared>();
    let (paired, code) = {
        let s = lk(&st.settings);
        (s.telegram_chat_id, s.telegram_pair_code.clone())
    };
    // Pairing: hanya chat yang mengirim kode dari Pengaturan yang boleh pakai bot.
    match paired {
        None => {
            let given = text.trim_start_matches("/start").trim();
            if !code.is_empty() && given == code {
                let mut s = lk(&st.settings);
                s.telegram_chat_id = Some(chat);
                st.store.save("settings.json", &*s);
                drop(s);
                let _ = app.emit("settings-changed", ());
                return Some("Terhubung ke Gaspol 🎉\nKirim kalimat apa pun untuk bikin tugas.\n/hari — daftar tugas\n/fokus — mulai sesi\n/stop — hentikan sesi\n/laporan — ringkasan hari ini".into());
            }
            return Some("Kirim kode pairing dari Gaspol → Pengaturan → Telegram.".into());
        }
        Some(c) if c != chat => return None,
        _ => {}
    }
    let cmd = text.split_whitespace().next().unwrap_or("").to_lowercase();
    match cmd.as_str() {
        "/hari" | "/today" | "/list" => {
            let tasks = crate::insights::urgent(app, 6);
            if tasks.is_empty() {
                return Some("List kosong. Mantap.".into());
            }
            Some(
                tasks
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let dl = t.deadline.as_deref().map(|d| format!(" · {d}")).unwrap_or_default();
                        let step = t.steps.iter().find(|s| !s.done).map(|s| format!("\n   → {}", s.text)).unwrap_or_default();
                        format!("{}. {}{dl}{step}", i + 1, t.title)
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }
        "/fokus" | "/focus" => {
            let id = crate::insights::urgent(app, 1).first().map(|t| t.id.clone());
            let v = crate::start_session_inner(app, id, None, None);
            Some(format!("Sesi {}m mulai: {}", v.planned / 60, v.task_title))
        }
        "/stop" => {
            crate::finish_session(app, false);
            Some("Sesi dihentikan.".into())
        }
        "/laporan" | "/report" => Some(crate::insights::daily_text(app)),
        "/start" | "/help" => Some("Kirim kalimat apa pun untuk bikin tugas.\n/hari · /fokus · /stop · /laporan".into()),
        _ => match crate::capture_text(app, text.to_string()).await {
            Ok(r) => {
                let dl = r.task.deadline.as_deref().map(|d| format!("\n📅 {d}")).unwrap_or_default();
                let step = r.task.steps.first().map(|s| format!("\n→ {}", s.text)).unwrap_or_default();
                Some(format!("✔ {}{dl}{step}", r.task.title))
            }
            Err(e) => Some(format!("Gagal: {e}")),
        },
    }
}

// ------------------------------------------------------------------ kalender ICS

#[derive(Serialize, Clone, Debug)]
pub struct CalEvent {
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
}

fn ics_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n")
}

/// Tulis `gaspol.ics`: deadline tugas (all-day) + sesi fokus 14 hari terakhir.
pub fn export_ics(app: &AppHandle) -> std::path::PathBuf {
    let st = app.state::<Shared>();
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Gaspol//ID\r\nX-WR-CALNAME:Gaspol\r\n");
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    for t in lk(&st.tasks).iter().filter(|t| !t.done) {
        let Some(d) = t.deadline.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()) else { continue };
        let next = d.succ_opt().unwrap_or(d);
        out += &format!(
            "BEGIN:VEVENT\r\nUID:{}@gaspol\r\nDTSTAMP:{stamp}\r\nDTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\nSUMMARY:⏰ {}\r\nEND:VEVENT\r\n",
            t.id,
            d.format("%Y%m%d"),
            next.format("%Y%m%d"),
            ics_escape(&t.title)
        );
    }
    let days = st.store.list_days();
    for date in days.iter().rev().take(14) {
        let d: crate::store::DayLog = st.store.load(&Store::day_name(date));
        for (i, s) in d.sessions.iter().enumerate() {
            let fmt = |ts: i64| chrono::Utc.timestamp_opt(ts, 0).single().map(|t| t.format("%Y%m%dT%H%M%SZ").to_string()).unwrap_or_default();
            out += &format!(
                "BEGIN:VEVENT\r\nUID:s{}-{i}@gaspol\r\nDTSTAMP:{stamp}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:🎯 {}\r\nEND:VEVENT\r\n",
                s.start,
                fmt(s.start),
                fmt(s.end.max(s.start + 60)),
                ics_escape(&s.task_title)
            );
        }
    }
    out += "END:VCALENDAR\r\n";
    let path = st.store.dir.join("gaspol.ics");
    let _ = std::fs::write(&path, out);
    path
}

fn parse_ics_time(prop: &str, value: &str) -> Option<(i64, bool)> {
    let v = value.trim();
    if prop.contains("VALUE=DATE") || v.len() == 8 {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        let dt = Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single()?;
        return Some((dt.timestamp(), true));
    }
    if let Some(utc) = v.strip_suffix('Z') {
        let dt = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((chrono::Utc.from_utc_datetime(&dt).timestamp(), false));
    }
    // TZID=... → anggap zona waktu lokal laptop.
    let dt = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    Some((Local.from_local_datetime(&dt).single()?.timestamp(), false))
}

pub fn parse_ics(text: &str) -> Vec<CalEvent> {
    // Unfold baris lanjutan (diawali spasi/tab).
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() {
            let last = lines.last_mut().unwrap();
            last.push_str(&raw[1..]);
        } else {
            lines.push(raw.to_string());
        }
    }
    let mut out = Vec::new();
    let mut cur: Option<(String, Option<(i64, bool)>, Option<(i64, bool)>)> = None;
    for l in lines {
        if l == "BEGIN:VEVENT" {
            cur = Some((String::new(), None, None));
        } else if l == "END:VEVENT" {
            if let Some((title, Some(s), e)) = cur.take() {
                let end = e.map(|e| e.0).unwrap_or(s.0 + if s.1 { 86400 } else { 3600 });
                out.push(CalEvent { title, start: s.0, end, all_day: s.1 });
            }
        } else if let Some(c) = cur.as_mut() {
            let Some((prop, value)) = l.split_once(':') else { continue };
            if prop.starts_with("SUMMARY") {
                c.0 = value.replace("\\,", ",").replace("\\;", ";").replace("\\n", " ");
            } else if prop.starts_with("DTSTART") {
                c.1 = parse_ics_time(prop, value);
            } else if prop.starts_with("DTEND") {
                c.2 = parse_ics_time(prop, value);
            }
        }
    }
    out
}

pub async fn refresh_calendar(app: &AppHandle) -> Result<usize, String> {
    let url = lk(&app.state::<Shared>().settings).ics_url.trim().to_string();
    if url.is_empty() {
        lk(&app.state::<Shared>().rt).cal.clear();
        return Ok(0);
    }
    let url = url.replacen("webcal://", "https://", 1);
    let text = client().get(&url).send().await.map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("URL bukan file kalender iCal".into());
    }
    let now = now_ts();
    let mut events: Vec<CalEvent> = parse_ics(&text)
        .into_iter()
        .filter(|e| e.end > now - 86400 && e.start < now + 8 * 86400)
        .collect();
    events.sort_by_key(|e| e.start);
    let n = events.len();
    let st = app.state::<Shared>();
    let mut rt = lk(&st.rt);
    rt.cal = events;
    rt.cal_error = None;
    rt.last_cal = now;
    Ok(n)
}

/// Loop latar: kalender tiap 30 menit, Git tiap 5 menit.
pub fn spawn_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut tick: u64 = 0;
        loop {
            if tick % 30 == 0 {
                if let Err(e) = refresh_calendar(&app).await {
                    lk(&app.state::<Shared>().rt).cal_error = Some(e);
                }
                export_ics(&app);
            }
            if tick % 5 == 1 {
                let a = app.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || poll_git(&a)).await;
            }
            tick += 1;
            tokio_sleep(60).await;
        }
    });
}

// ------------------------------------------------------------------ Git

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(String::from)
        .collect()
}

/// Commit baru → dicatat di log harian, menyentuh tugas di project yang sama,
/// dan mencentang langkah yang isinya mirip pesan commit.
pub fn poll_git(app: &AppHandle) {
    let st = app.state::<Shared>();
    let repos = lk(&st.settings).git_repos.clone();
    if repos.is_empty() {
        return;
    }
    let since = {
        let mut rt = lk(&st.rt);
        let s = if rt.last_git == 0 { now_ts() - 6 * 3600 } else { rt.last_git };
        rt.last_git = now_ts();
        s
    };
    let mut fresh = Vec::new();
    for r in repos.iter().filter(|r| !r.path.trim().is_empty()) {
        let out = win::command("git")
            .args(["-C", r.path.trim(), "log", "--all", &format!("--since={since}"), "--pretty=format:%H\x1f%ct\x1f%s"])
            .output();
        let Ok(out) = out else { continue };
        let name = std::path::Path::new(r.path.trim())
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let project = if r.project.trim().is_empty() { name.clone() } else { r.project.trim().to_string() };
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let p: Vec<&str> = line.split('\x1f').collect();
            if p.len() < 3 {
                continue;
            }
            fresh.push(CommitLog {
                repo: name.clone(),
                project: project.clone(),
                hash: p[0].chars().take(10).collect(),
                ts: p[1].parse().unwrap_or(0),
                message: p[2].to_string(),
            });
        }
    }
    if fresh.is_empty() {
        return;
    }
    {
        let mut d = lk(&st.day);
        for c in &fresh {
            if !d.commits.iter().any(|x| x.hash == c.hash) {
                d.commits.push(c.clone());
            }
        }
    }
    let mut changed = false;
    {
        let mut tasks = lk(&st.tasks);
        for c in &fresh {
            let cw = words(&c.message);
            for t in tasks.iter_mut().filter(|t| !t.done) {
                let same_project = t.project.as_deref().map(|p| p.eq_ignore_ascii_case(&c.project)).unwrap_or(false);
                if !same_project {
                    continue;
                }
                t.last_touched = Some(now_ts().max(c.ts));
                changed = true;
                for s in t.steps.iter_mut().filter(|s| !s.done) {
                    let sw = words(&s.text);
                    if sw.is_empty() {
                        continue;
                    }
                    let hit = sw.iter().filter(|w| cw.contains(w)).count();
                    if hit * 10 >= sw.len() * 6 {
                        s.done = true;
                    }
                }
            }
        }
        if changed {
            st.store.save("tasks.json", &*tasks);
        }
    }
    if changed {
        crate::emit_tasks(app);
    }
}
