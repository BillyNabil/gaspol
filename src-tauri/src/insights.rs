//! Angka-angka: estimasi yang belajar, rencana pagi, laporan mingguan, XP,
//! rekap per klien + tagihan, dan update status untuk klien.

use crate::store::{DayLog, Store, Task};
use crate::{ai, lk, now_ts, secret_get, today_str, Shared};
use chrono::{Duration, Local, NaiveDate, TimeZone};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};

fn spent(t: &Task) -> u64 {
    t.focus_secs + t.auto_secs
}

/// Seberapa meleset estimasimu biasanya (aktual / estimasi), median dari tugas selesai.
pub fn estimate_ratio(tasks: &[Task]) -> f64 {
    let mut r: Vec<f64> = tasks
        .iter()
        .filter(|t| t.done && spent(t) >= 600)
        .filter_map(|t| Some(spent(t) as f64 / 60.0 / t.est_minutes.filter(|m| *m >= 10)? as f64))
        .collect();
    if r.len() < 3 {
        return 1.0;
    }
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    r[r.len() / 2].clamp(0.5, 4.0)
}

pub fn remaining_minutes(t: &Task, ratio: f64) -> Option<u32> {
    let est = t.est_minutes? as f64 * ratio;
    Some((est - spent(t) as f64 / 60.0).max(15.0).round() as u32)
}

fn days_left(t: &Task, today: NaiveDate) -> i64 {
    t.deadline
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .map(|d| (d - today).num_days())
        .unwrap_or(99)
}

/// Tugas terbuka, paling mendesak dulu.
pub fn urgent(app: &AppHandle, n: usize) -> Vec<Task> {
    let today = Local::now().date_naive();
    let mut open: Vec<Task> = lk(&app.state::<Shared>().tasks).iter().filter(|t| !t.done).cloned().collect();
    open.sort_by(|a, b| {
        days_left(a, today)
            .cmp(&days_left(b, today))
            .then(b.snoozes.cmp(&a.snoozes))
            .then(b.created_at.cmp(&a.created_at))
    });
    open.truncate(n);
    open
}

#[derive(Serialize, Clone)]
pub struct PlanItem {
    pub id: String,
    pub title: String,
    pub step: Option<String>,
    pub deadline: Option<String>,
    pub need_min: Option<u32>,
}

#[derive(Serialize, Clone)]
pub struct Plan {
    pub tasks: Vec<PlanItem>,
    pub free: Vec<(i64, i64)>,
    pub free_text: Option<String>,
    pub events: Vec<crate::automation::CalEvent>,
}

fn hm(ts: i64) -> String {
    Local.timestamp_opt(ts, 0).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

pub fn plan(app: &AppHandle) -> Plan {
    let st = app.state::<Shared>();
    let ratio = estimate_ratio(&lk(&st.tasks));
    let tasks = urgent(app, 3)
        .into_iter()
        .map(|t| PlanItem {
            need_min: remaining_minutes(&t, ratio),
            step: t.steps.iter().find(|s| !s.done).map(|s| s.text.clone()),
            id: t.id,
            title: t.title,
            deadline: t.deadline,
        })
        .collect();
    // Slot kosong hari ini dari kalender (#16).
    let now = Local::now();
    let day_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let base = Local.from_local_datetime(&day_start).single().map(|d| d.timestamp()).unwrap_or(0);
    let from = now.timestamp().max(base + 8 * 3600);
    let to = base + 22 * 3600;
    let events: Vec<_> = lk(&st.rt)
        .cal
        .iter()
        .filter(|e| e.end > base && e.start < base + 86400)
        .cloned()
        .collect();
    let mut busy: Vec<(i64, i64)> = events.iter().filter(|e| !e.all_day).map(|e| (e.start, e.end)).collect();
    busy.sort();
    let mut free = Vec::new();
    let mut cur = from;
    for (s, e) in busy {
        if s > cur && s - cur >= 45 * 60 {
            free.push((cur, s.min(to)));
        }
        cur = cur.max(e);
        if cur >= to {
            break;
        }
    }
    if to - cur >= 45 * 60 {
        free.push((cur, to));
    }
    let free_text = (!lk(&st.rt).cal.is_empty() && !free.is_empty()).then(|| {
        free.iter().take(3).map(|(a, b)| format!("{}–{}", hm(*a), hm(*b))).collect::<Vec<_>>().join(", ")
    });
    Plan { tasks, free, free_text, events }
}

fn fmt_dur(secs: u64) -> String {
    let m = (secs + 30) / 60;
    if m >= 60 {
        format!("{}j {}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

fn load_day(app: &AppHandle, date: &str) -> DayLog {
    let st = app.state::<Shared>();
    let cur = lk(&st.day);
    if cur.date == date {
        return cur.clone();
    }
    drop(cur);
    let mut d: DayLog = st.store.load(&Store::day_name(date));
    d.date = date.to_string();
    d
}

fn day_focus(d: &DayLog) -> u64 {
    d.sessions.iter().map(|s| s.focus_secs).sum::<u64>() + d.task_secs.values().sum::<u64>()
}

pub fn daily_text(app: &AppHandle) -> String {
    let d = load_day(app, &today_str());
    let total: u64 = d.apps.values().map(|a| a.secs).sum();
    let dis: u64 = d.apps.values().filter(|a| a.distracting).map(|a| a.secs).sum();
    let top = d
        .apps
        .iter()
        .filter(|(_, a)| !a.distracting)
        .max_by_key(|(_, a)| a.secs)
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| "-".into());
    let s = stats(app);
    format!(
        "📊 Gaspol {}\nFokus {} · ngelantur {} ({}×)\nAktif {} · paling lama di {}\nTugas tuntas: {} · commit: {}\n🔥 Streak {} hari · Level {}",
        today_str(),
        fmt_dur(day_focus(&d)),
        fmt_dur(dis),
        d.distractions,
        fmt_dur(total),
        top,
        d.tasks_done,
        d.commits.len(),
        s.streak,
        s.level
    )
}

#[derive(Serialize, Default, Clone)]
pub struct Stats {
    pub xp: u64,
    pub level: u32,
    pub level_from: u64,
    pub level_to: u64,
    pub streak: u32,
    pub ratio: f64,
    pub samples: usize,
}

fn level_for(xp: u64) -> (u32, u64, u64) {
    // Level n butuh 60·n² XP total (1 XP ≈ 1 menit fokus).
    let mut n = 1u64;
    while 60 * n * n <= xp {
        n += 1;
    }
    (n as u32, 60 * (n - 1) * (n - 1), 60 * n * n)
}

pub fn stats(app: &AppHandle) -> Stats {
    let st = app.state::<Shared>();
    let days = st.store.list_days();
    let today = today_str();
    let mut xp = 0u64;
    let mut focus_by_day: BTreeMap<String, u64> = BTreeMap::new();
    for date in days.iter().chain(std::iter::once(&today)) {
        if focus_by_day.contains_key(date) {
            continue;
        }
        let d = load_day(app, date);
        let f = day_focus(&d);
        xp += d.sessions.iter().map(|s| s.focus_secs / 60).sum::<u64>()
            + d.task_secs.values().sum::<u64>() / 120
            + d.sessions.iter().filter(|s| s.completed).count() as u64 * 20;
        focus_by_day.insert(date.clone(), f);
    }
    let tasks = lk(&st.tasks);
    xp += tasks.iter().filter(|t| t.done).count() as u64 * 50;
    // Streak: hari berturut-turut dengan ≥ 25 menit fokus (hari ini belum dihitung kalau belum cukup).
    let mut streak = 0;
    let mut day = Local::now().date_naive();
    if focus_by_day.get(&today).copied().unwrap_or(0) < 1500 {
        day -= Duration::days(1);
    }
    loop {
        let key = day.format("%Y-%m-%d").to_string();
        if focus_by_day.get(&key).copied().unwrap_or(0) >= 1500 {
            streak += 1;
            day -= Duration::days(1);
        } else {
            break;
        }
    }
    let samples = tasks.iter().filter(|t| t.done && spent(t) >= 600 && t.est_minutes.unwrap_or(0) >= 10).count();
    let (level, from, to) = level_for(xp);
    Stats { xp, level, level_from: from, level_to: to, streak, ratio: estimate_ratio(&tasks), samples }
}

#[derive(Serialize)]
pub struct WeekDay {
    pub date: String,
    pub focus: u64,
    pub distracted: u64,
    pub total: u64,
    pub sessions: usize,
    pub commits: usize,
    pub renders: u32,
}

#[derive(Serialize)]
pub struct Week {
    pub days: Vec<WeekDay>,
    pub heat: Vec<Vec<u64>>,
    pub heat_dis: Vec<Vec<u64>>,
    pub best: Option<String>,
    pub focus: u64,
    pub distracted: u64,
    pub prev_focus: u64,
    pub stats: Stats,
}

/// 7 hari sampai `end` (#21).
pub fn week(app: &AppHandle, end: &str) -> Week {
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive());
    let mut days = Vec::new();
    let mut heat = Vec::new();
    let mut heat_dis = Vec::new();
    let mut hours_sum = [0u64; 24];
    for i in (0..7).rev() {
        let date = (end - Duration::days(i)).format("%Y-%m-%d").to_string();
        let d = load_day(app, &date);
        let mut h = d.hours.clone();
        h.resize(24, 0);
        let mut hd = d.hours_dis.clone();
        hd.resize(24, 0);
        for (k, v) in h.iter().enumerate() {
            hours_sum[k] += v;
        }
        days.push(WeekDay {
            focus: day_focus(&d),
            distracted: d.apps.values().filter(|a| a.distracting).map(|a| a.secs).sum(),
            total: d.apps.values().map(|a| a.secs).sum(),
            sessions: d.sessions.len(),
            commits: d.commits.len(),
            renders: d.renders,
            date,
        });
        heat.push(h);
        heat_dis.push(hd);
    }
    // Jendela 3 jam paling produktif.
    let mut best = None;
    let mut best_v = 0;
    for h in 0..22 {
        let v = hours_sum[h] + hours_sum[h + 1] + hours_sum[h + 2];
        if v > best_v {
            best_v = v;
            best = Some(format!("{h:02}:00–{:02}:00", h + 3));
        }
    }
    if best_v < 3600 {
        best = None;
    }
    let mut prev_focus = 0;
    for i in 7..14 {
        let date = (end - Duration::days(i)).format("%Y-%m-%d").to_string();
        prev_focus += day_focus(&load_day(app, &date));
    }
    Week {
        focus: days.iter().map(|d| d.focus).sum(),
        distracted: days.iter().map(|d| d.distracted).sum(),
        days,
        heat,
        heat_dis,
        best,
        prev_focus,
        stats: stats(app),
    }
}

// ------------------------------------------------------------------ klien (#18)

#[derive(Serialize, Clone)]
pub struct ClientRow {
    pub project: String,
    pub secs: u64,
    pub rate: f64,
    pub amount: f64,
    pub sessions: usize,
    pub done: Vec<String>,
}

pub fn clients(app: &AppHandle, from: &str, to: &str) -> Vec<ClientRow> {
    let st = app.state::<Shared>();
    let tasks = lk(&st.tasks).clone();
    let rates = lk(&st.settings).rates.clone();
    let project_of = |id: &str| tasks.iter().find(|t| t.id == id).and_then(|t| t.project.clone());
    let mut rows: BTreeMap<String, ClientRow> = BTreeMap::new();
    let mut add = |p: String, secs: u64, session: bool| {
        let r = rows.entry(p.clone()).or_insert(ClientRow { project: p, secs: 0, rate: 0.0, amount: 0.0, sessions: 0, done: vec![] });
        r.secs += secs;
        if session {
            r.sessions += 1;
        }
    };
    let (Ok(a), Ok(b)) = (NaiveDate::parse_from_str(from, "%Y-%m-%d"), NaiveDate::parse_from_str(to, "%Y-%m-%d")) else {
        return vec![];
    };
    let mut d = a;
    while d <= b {
        let log = load_day(app, &d.format("%Y-%m-%d").to_string());
        for s in &log.sessions {
            let p = s.project.clone().or_else(|| s.task_id.as_deref().and_then(project_of));
            if let Some(p) = p {
                add(p, s.focus_secs, true);
            }
        }
        for (id, secs) in &log.task_secs {
            if let Some(p) = project_of(id) {
                add(p, *secs, false);
            }
        }
        d += Duration::days(1);
    }
    let (fa, fb) = (
        Local.from_local_datetime(&a.and_hms_opt(0, 0, 0).unwrap()).single().map(|x| x.timestamp()).unwrap_or(0),
        Local.from_local_datetime(&b.and_hms_opt(23, 59, 59).unwrap()).single().map(|x| x.timestamp()).unwrap_or(0),
    );
    for t in tasks.iter().filter(|t| t.done && t.done_at.map(|x| x >= fa && x <= fb).unwrap_or(false)) {
        if let Some(p) = &t.project {
            if let Some(r) = rows.get_mut(p) {
                r.done.push(t.title.clone());
            } else {
                rows.insert(p.clone(), ClientRow { project: p.clone(), secs: 0, rate: 0.0, amount: 0.0, sessions: 0, done: vec![t.title.clone()] });
            }
        }
    }
    let mut out: Vec<ClientRow> = rows
        .into_values()
        .map(|mut r| {
            r.rate = rates.get(&r.project).copied().unwrap_or(0.0);
            r.amount = (r.secs as f64 / 3600.0 * r.rate).round();
            r
        })
        .collect();
    out.sort_by(|a, b| b.secs.cmp(&a.secs));
    out
}

fn downloads() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
    let p = std::path::Path::new(&home).join("Downloads");
    if p.exists() {
        p
    } else {
        std::path::PathBuf::from(home)
    }
}

fn money(v: f64, cur: &str) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    format!("{cur} {}{out}", if n < 0 { "-" } else { "" })
}

pub fn export_csv(app: &AppHandle, from: &str, to: &str) -> Result<String, String> {
    let rows = clients(app, from, to);
    let mut csv = String::from("project,jam,sesi,rate_per_jam,total,tugas_selesai\n");
    for r in &rows {
        csv += &format!(
            "\"{}\",{:.2},{},{},{},\"{}\"\n",
            r.project.replace('"', "\"\""),
            r.secs as f64 / 3600.0,
            r.sessions,
            r.rate,
            r.amount,
            r.done.join("; ").replace('"', "\"\"")
        );
    }
    let path = downloads().join(format!("gaspol-klien_{from}_{to}.csv"));
    std::fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Invoice HTML siap print → PDF (Ctrl+P di browser).
pub fn invoice(app: &AppHandle, project: &str, from: &str, to: &str) -> Result<String, String> {
    let rows = clients(app, from, to);
    let r = rows.iter().find(|r| r.project == project).ok_or("Tidak ada data untuk project ini")?;
    let cur = lk(&app.state::<Shared>().settings).currency.clone();
    let hours = r.secs as f64 / 3600.0;
    let items: String = if r.done.is_empty() {
        "<li>Pengerjaan project</li>".into()
    } else {
        r.done.iter().map(|t| format!("<li>{}</li>", html_escape(t))).collect()
    };
    let no = format!("INV-{}-{}", Local::now().format("%Y%m%d"), project.chars().filter(|c| c.is_alphanumeric()).take(6).collect::<String>().to_uppercase());
    let html = format!(
        r#"<!doctype html><html lang="id"><head><meta charset="utf-8"><title>{no}</title>
<style>
body{{font-family:"Segoe UI",system-ui,sans-serif;color:#111;max-width:720px;margin:48px auto;padding:0 24px}}
h1{{font-size:28px;margin:0}} .muted{{color:#666}} table{{width:100%;border-collapse:collapse;margin-top:28px}}
td,th{{padding:10px 8px;border-bottom:1px solid #ddd;text-align:left}} th{{font-size:12px;color:#666;text-transform:uppercase}}
.r{{text-align:right}} .total td{{font-weight:700;font-size:18px;border-bottom:0}} ul{{margin:6px 0 0;padding-left:18px;color:#444}}
@media print{{.hint{{display:none}}}}
</style></head><body>
<p class="hint muted">Tekan Ctrl+P → Simpan sebagai PDF.</p>
<h1>Invoice</h1>
<p class="muted">{no} · {date}</p>
<p><b>Untuk:</b> {proj}<br><b>Periode:</b> {from} s/d {to}</p>
<table><tr><th>Deskripsi</th><th class="r">Jam</th><th class="r">Rate</th><th class="r">Jumlah</th></tr>
<tr><td>Pekerjaan untuk {proj}<ul>{items}</ul></td><td class="r">{hours:.1}</td><td class="r">{rate}</td><td class="r">{amount}</td></tr>
<tr class="total"><td colspan="3">Total</td><td class="r">{amount}</td></tr></table>
<p class="muted" style="margin-top:40px">Jam dihitung otomatis oleh Gaspol dari sesi fokus & pencatatan aktivitas.</p>
</body></html>"#,
        date = Local::now().format("%d %B %Y"),
        proj = html_escape(project),
        rate = money(r.rate, &cur),
        amount = money(r.amount, &cur),
    );
    let path = downloads().join(format!("{no}.html"));
    std::fs::write(&path, html).map_err(|e| e.to_string())?;
    crate::win::shell_open(&path.display().to_string());
    Ok(path.display().to_string())
}

/// Update progres singkat untuk dikirim ke klien (#20).
pub async fn client_update(app: &AppHandle, project: &str) -> Result<String, String> {
    let st = app.state::<Shared>();
    let tasks: Vec<Task> = lk(&st.tasks)
        .iter()
        .filter(|t| t.project.as_deref() == Some(project))
        .cloned()
        .collect();
    if tasks.is_empty() {
        return Err("Belum ada tugas untuk project ini".into());
    }
    let week_ago = now_ts() - 7 * 86400;
    let mut facts = String::new();
    for t in &tasks {
        let done_steps: Vec<&str> = t.steps.iter().filter(|s| s.done).map(|s| s.text.as_str()).collect();
        let next = t.steps.iter().find(|s| !s.done).map(|s| s.text.as_str());
        if t.done && t.done_at.unwrap_or(0) < week_ago {
            continue;
        }
        facts += &format!(
            "- {}{}{}{}\n",
            t.title,
            if t.done { " [SELESAI]" } else { "" },
            if done_steps.is_empty() { String::new() } else { format!("; sudah: {}", done_steps.join(", ")) },
            next.map(|n| format!("; berikutnya: {n}")).unwrap_or_default()
        );
    }
    let settings = lk(&st.settings).clone();
    if settings.ai_provider != "none" {
        let system = "Tulis update progres singkat (maks 6 kalimat) untuk klien freelancer, bahasa Indonesia sopan tapi santai, tanpa markdown berat, tanpa menyebut jam kerja. Akhiri dengan langkah berikutnya & perkiraan kapan ada update. Balas teksnya saja.";
        if let Ok(t) = ai::complete(&settings, secret_get(&settings.ai_provider), system, &format!("Project: {project}\n{facts}")).await {
            let t = match t.find("</think>") {
                Some(i) => t[i + 8..].trim().to_string(),
                None => t.trim().to_string(),
            };
            if !t.is_empty() {
                return Ok(t);
            }
        }
    }
    let mut out = format!("Halo! Update progres {project}:\n");
    for t in &tasks {
        if t.done && t.done_at.unwrap_or(0) < week_ago {
            continue;
        }
        let total = t.steps.len();
        let done = t.steps.iter().filter(|s| s.done).count();
        out += &format!(
            "• {} — {}\n",
            t.title,
            if t.done { "selesai ✅".to_string() } else if total > 0 { format!("{done}/{total} tahap") } else { "sedang dikerjakan".into() }
        );
    }
    if let Some(n) = tasks.iter().filter(|t| !t.done).find_map(|t| t.steps.iter().find(|s| !s.done)) {
        out += &format!("Berikutnya: {}. Nanti saya kabari lagi.", n.text.to_lowercase());
    }
    Ok(out)
}
