//! Satu kalimat → task + langkah kecil. Claude API, server lokal (OpenAI-compatible),
//! atau parser bawaan kalau AI mati / gagal.

use crate::store::Settings;
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde_json::{json, Value};

const HARI: [&str; 7] = ["Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu"];

pub struct Draft {
    pub title: String,
    pub project: Option<String>,
    pub deadline: Option<String>,
    pub est_minutes: Option<u32>,
    pub steps: Vec<String>,
}

fn system_prompt() -> String {
    let today = Local::now().date_naive();
    format!(
        r#"Kamu mesin pemecah tugas di aplikasi produktivitas. User mengetik satu kalimat singkat (campur Indonesia/Inggris, banyak singkatan) tentang hal yang harus dia kerjakan. User adalah freelancer kreatif dan developer (coding, Blender, After Effects, Roblox, klien).

Hari ini {date} ({dow}).

Balas HANYA satu objek JSON valid, tanpa teks lain dan tanpa markdown:
{{"title": "judul tugas singkat dan jelas, tanpa info deadline", "project": "nama klien/project kalau disebut, kalau tidak null", "deadline": "YYYY-MM-DD atau null", "est_minutes": perkiraan total menit (angka) atau null, "steps": ["3-5 langkah kecil konkret"]}}

Aturan steps:
- Langkah pertama HARUS bisa dimulai dalam 2 menit (contoh: "Buka file .blend scene final"), bukan "rencanakan" atau "pikirkan".
- Tiap langkah diawali kata kerja, maksimal 10 kata, bahasa Indonesia santai.
Aturan deadline: nama hari (jumat, senin, dst) = hari itu yang terdekat mulai hari ini; "besok" = +1 hari; "lusa" = +2 hari."#,
        date = today.format("%Y-%m-%d"),
        dow = HARI[today.weekday().num_days_from_monday() as usize]
    )
}

pub async fn draft(settings: &Settings, key: Option<String>, text: &str) -> Result<Draft, String> {
    let raw = complete(settings, key, &system_prompt(), text).await?;
    parse_draft(&raw).ok_or_else(|| "balasan AI tidak bisa dibaca".to_string())
}

pub async fn complete(settings: &Settings, key: Option<String>, system: &str, user: &str) -> Result<String, String> {
    match settings.ai_provider.as_str() {
        "claude" => {
            let key = key.ok_or("API key Claude belum diisi")?;
            anthropic("https://api.anthropic.com", &key, &settings.claude_model, false, system, user).await
        }
        "local" => openai(&settings.local_base_url, &settings.local_model, key, system, user).await,
        "custom" => {
            let base = settings.custom_base_url.trim();
            if base.is_empty() {
                return Err("Base URL custom provider belum diisi".into());
            }
            if settings.custom_format == "anthropic" {
                let key = key.ok_or("API key custom provider belum diisi")?;
                if settings.custom_model.trim().is_empty() {
                    return Err("Nama model custom provider belum diisi".into());
                }
                anthropic(base, &key, settings.custom_model.trim(), true, system, user).await
            } else {
                openai(base, &settings.custom_model, key, system, user).await
            }
        }
        _ => Err("AI nonaktif".into()),
    }
}

/// Terima base URL dalam bentuk apa pun: `https://x`, `https://x/v1`, atau URL endpoint lengkap.
fn anthropic_url(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    if b.ends_with("/messages") {
        b.to_string()
    } else if b.ends_with("/v1") {
        format!("{b}/messages")
    } else {
        format!("{b}/v1/messages")
    }
}

fn openai_base(base: &str) -> &str {
    let b = base.trim().trim_end_matches('/');
    b.strip_suffix("/chat/completions").unwrap_or(b)
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

fn net_err(e: reqwest::Error) -> String {
    if e.is_connect() {
        "tidak bisa konek ke server AI (sudah dinyalakan?)".into()
    } else if e.is_timeout() {
        "AI kelamaan merespons".into()
    } else {
        e.to_string()
    }
}

async fn read_json(res: reqwest::Response) -> Result<Value, String> {
    let status = res.status();
    let v: Value = res.json().await.map_err(|e| format!("respons tidak valid: {e}"))?;
    if !status.is_success() {
        let msg = v["error"]["message"]
            .as_str()
            .or_else(|| v["error"].as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(msg);
    }
    Ok(v)
}

async fn anthropic(base: &str, key: &str, model: &str, bearer: bool, system: &str, user: &str) -> Result<String, String> {
    let body = json!({
        "model": model,
        "max_tokens": 800,
        "system": system,
        "messages": [{ "role": "user", "content": user }],
    });
    let mut req = client()?
        .post(anthropic_url(base))
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);
    // Banyak proxy Anthropic-compatible minta Bearer, bukan x-api-key.
    if bearer {
        req = req.bearer_auth(key);
    }
    let res = req.send().await.map_err(net_err)?;
    let v = read_json(res).await?;
    let text: String = v["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect();
    Ok(text)
}

async fn openai(base: &str, model: &str, key: Option<String>, system: &str, user: &str) -> Result<String, String> {
    let base = openai_base(base);
    let model = if model.trim().is_empty() {
        list_models(base, key.clone())
            .await?
            .into_iter()
            .next()
            .ok_or("belum ada model yang tersedia di server")?
    } else {
        model.trim().to_string()
    };
    let body = json!({
        "model": model,
        "temperature": 0.2,
        "max_tokens": 1500,
        "stream": false,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });
    let mut req = client()?.post(format!("{base}/chat/completions")).json(&body);
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let v = read_json(req.send().await.map_err(net_err)?).await?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "server tidak mengembalikan teks".into())
}

pub async fn list_models(base: &str, key: Option<String>) -> Result<Vec<String>, String> {
    let base = openai_base(base);
    let mut req = client()?.get(format!("{base}/models"));
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let v = read_json(req.send().await.map_err(net_err)?).await?;
    Ok(v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().map(String::from))
        .filter(|id| !id.to_lowercase().contains("embed"))
        .collect())
}

fn parse_draft(raw: &str) -> Option<Draft> {
    let text = match raw.find("</think>") {
        Some(i) => &raw[i + 8..],
        None => raw,
    };
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let v: Value = serde_json::from_str(text.get(start..=end)?).ok()?;
    let title = v["title"].as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let project = v["project"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.to_lowercase() != "null");
    let deadline = v["deadline"]
        .as_str()
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
        .map(|d| d.format("%Y-%m-%d").to_string());
    let est_minutes = v["est_minutes"]
        .as_f64()
        .or_else(|| v["est_minutes"].as_str().and_then(|s| s.trim().parse().ok()))
        .map(|f| f.round() as u32)
        .filter(|m| *m > 0);
    let steps = v["steps"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .take(6)
                .collect()
        })
        .unwrap_or_default();
    Some(Draft { title, project, deadline, est_minutes, steps })
}

// ---------------------------------------------------------------- parser bawaan

const CONNECT: &[&str] = &[
    "sebelum", "sblm", "sampai", "sampe", "smpe", "deadline", "dl", "before", "by", "tgl", "tanggal", "paling",
    "lambat", "due", "pada", "hari", "on",
];
const PROJECT_WORDS: &[&str] = &["klien", "client", "project", "proyek", "projek"];

fn weekday_idx(w: &str) -> Option<u32> {
    Some(match w {
        "senin" | "monday" => 0,
        "selasa" | "tuesday" => 1,
        "rabu" | "rab" | "wednesday" => 2,
        "kamis" | "kam" | "thursday" => 3,
        "jumat" | "jum'at" | "jum" | "friday" => 4,
        "sabtu" | "sab" | "saturday" => 5,
        "minggu" | "ahad" | "sunday" => 6,
        _ => return None,
    })
}

fn parse_dm(w: &str, today: NaiveDate) -> Option<NaiveDate> {
    let parts: Vec<&str> = w.split(['/', '-']).collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let d: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let y: i32 = match parts.get(2) {
        Some(y) => {
            let y: i32 = y.parse().ok()?;
            if y < 100 {
                2000 + y
            } else {
                y
            }
        }
        None => today.year(),
    };
    let date = NaiveDate::from_ymd_opt(y, m, d)?;
    if parts.len() == 2 && date < today {
        return NaiveDate::from_ymd_opt(y + 1, m, d);
    }
    Some(date)
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn heuristic(text: &str) -> Draft {
    let today = Local::now().date_naive();
    let words: Vec<&str> = text.split_whitespace().collect();
    let norm: Vec<String> = words
        .iter()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '\'')
                .to_string()
        })
        .collect();
    let mut used = vec![false; words.len()];
    let mut deadline = None;
    let mut project = None;
    let mut i = 0;
    while i < norm.len() {
        let w = norm[i].as_str();
        let next = norm.get(i + 1).map(|s| s.as_str()).unwrap_or("");
        let hit: Option<(NaiveDate, usize)> = if matches!(w, "hari" | "malam" | "sore" | "siang" | "pagi") && next == "ini" {
            Some((today, 2))
        } else if w == "minggu" && next == "depan" {
            Some((today + Duration::days(7), 2))
        } else {
            match w {
                "besok" | "bsk" | "besuk" | "tomorrow" => Some((today + Duration::days(1), 1)),
                "lusa" => Some((today + Duration::days(2), 1)),
                "today" | "hariini" | "skrg" | "sekarang" | "tonight" => Some((today, 1)),
                _ => weekday_idx(w)
                    .map(|idx| {
                        let cur = today.weekday().num_days_from_monday();
                        (today + Duration::days(((idx + 7 - cur) % 7) as i64), 1)
                    })
                    .or_else(|| parse_dm(w, today).map(|d| (d, 1))),
            }
        };
        if let Some((d, n)) = hit {
            deadline = Some(d);
            for u in used.iter_mut().skip(i).take(n) {
                *u = true;
            }
            let mut j = i;
            while j > 0 && !used[j - 1] && CONNECT.contains(&norm[j - 1].as_str()) {
                used[j - 1] = true;
                j -= 1;
            }
            i += n;
            continue;
        }
        if project.is_none() && PROJECT_WORDS.contains(&w) && i + 1 < words.len() {
            project = Some(format!("{} {}", cap(w), words[i + 1]));
        }
        i += 1;
    }
    let mut title: String = words
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(w, _)| *w)
        .collect::<Vec<_>>()
        .join(" ");
    if title.trim().is_empty() {
        title = text.trim().to_string();
    }
    let title = cap(title.trim());
    let short: String = title.chars().take(48).collect();
    Draft {
        steps: vec![
            format!("Buka semua file dan tools untuk: {short}"),
            "Kerjakan 10 menit pertama tanpa pindah tab".into(),
            "Tulis langkah berikutnya sebelum istirahat".into(),
        ],
        title,
        project,
        deadline: deadline.map(|d| d.format("%Y-%m-%d").to_string()),
        est_minutes: None,
    }
}
