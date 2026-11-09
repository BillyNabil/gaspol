//! Satu kalimat → task + langkah kecil. Claude API, server lokal (OpenAI-compatible),
//! atau parser bawaan kalau AI mati / gagal.

use crate::store::{Settings, Template};
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
    draft_with(settings, key, text, None).await
}

pub async fn draft_with(settings: &Settings, key: Option<String>, text: &str, tpl: Option<&Template>) -> Result<Draft, String> {
    let mut system = system_prompt();
    if let Some(t) = tpl {
        system += &format!(
            "\n\nTugas ini jenis \"{}\". Pakai kerangka langkah berikut sebagai acuan (sesuaikan dengan isi kalimat, 4-6 langkah): {}",
            t.name,
            t.steps.join(" -> ")
        );
    }
    let raw = complete(settings, key, &system, text).await?;
    parse_draft(&raw, 6).ok_or_else(|| "balasan AI tidak bisa dibaca".to_string())
}

/// Screenshot chat/brief klien -> satu tugas berisi checklist revisi (#12).
pub async fn draft_image(settings: &Settings, key: Option<String>, b64: &str, mime: &str, note: &str) -> Result<Draft, String> {
    let system = format!(
        "{}\n\nKali ini user mengirim GAMBAR (screenshot chat klien/dosen, brief, atau catatan revisi). Ubah isinya jadi SATU tugas: title = ringkasan pekerjaannya, steps = checklist revisi/permintaan satu per satu sesuai gambar (boleh sampai 12 langkah, urut seperti di gambar). Deadline hanya kalau disebut di gambar atau catatan user.",
        system_prompt()
    );
    let note = if note.trim().is_empty() { "Ubah gambar ini jadi tugas + checklist." } else { note };
    let raw = complete_inner(settings, key, &system, Content::Image { text: note, b64, mime }).await?;
    parse_draft(&raw, 12).ok_or_else(|| "balasan AI tidak bisa dibaca".to_string())
}

/// Template yang kata kuncinya muncul di kalimat (#17).
pub fn find_template<'a>(templates: &'a [Template], text: &str) -> Option<&'a Template> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect();
    templates
        .iter()
        .map(|t| (t, t.keywords.iter().filter(|k| words.contains(&k.trim().to_lowercase())).count()))
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(t, _)| t)
}

pub async fn complete(settings: &Settings, key: Option<String>, system: &str, user: &str) -> Result<String, String> {
    complete_inner(settings, key, system, Content::Text(user)).await
}

enum Content<'a> {
    Text(&'a str),
    Image { text: &'a str, b64: &'a str, mime: &'a str },
}

impl Content<'_> {
    fn anthropic(&self) -> Value {
        match self {
            Content::Text(t) => json!(t),
            Content::Image { text, b64, mime } => json!([
                { "type": "image", "source": { "type": "base64", "media_type": mime, "data": b64 } },
                { "type": "text", "text": text },
            ]),
        }
    }
    fn openai(&self) -> Value {
        match self {
            Content::Text(t) => json!(t),
            Content::Image { text, b64, mime } => json!([
                { "type": "text", "text": text },
                { "type": "image_url", "image_url": { "url": format!("data:{mime};base64,{b64}") } },
            ]),
        }
    }
}

async fn complete_inner(settings: &Settings, key: Option<String>, system: &str, user: Content<'_>) -> Result<String, String> {
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

async fn anthropic(base: &str, key: &str, model: &str, bearer: bool, system: &str, user: Content<'_>) -> Result<String, String> {
    let body = json!({
        "model": model,
        "max_tokens": 1500,
        "system": system,
        "messages": [{ "role": "user", "content": user.anthropic() }],
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

async fn openai(base: &str, model: &str, key: Option<String>, system: &str, user: Content<'_>) -> Result<String, String> {
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
            { "role": "user", "content": user.openai() },
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

/// Suara -> teks lewat endpoint OpenAI-compatible `/audio/transcriptions` (#13).
pub async fn transcribe(settings: &Settings, key: Option<String>, audio: Vec<u8>, mime: &str) -> Result<String, String> {
    let base = if !settings.stt_base_url.trim().is_empty() {
        settings.stt_base_url.trim().to_string()
    } else if settings.ai_provider == "custom" && settings.custom_format == "openai" {
        settings.custom_base_url.trim().to_string()
    } else {
        return Err("Isi server transkripsi suara di Pengaturan -> AI".into());
    };
    let ext = if mime.contains("ogg") { "ogg" } else if mime.contains("wav") { "wav" } else if mime.contains("mp4") { "m4a" } else { "webm" };
    let part = reqwest::multipart::Part::bytes(audio)
        .file_name(format!("voice.{ext}"))
        .mime_str(mime.split(';').next().unwrap_or("audio/webm"))
        .map_err(|e| e.to_string())?;
    let model = if settings.stt_model.trim().is_empty() { "whisper-1".to_string() } else { settings.stt_model.trim().to_string() };
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model)
        .text("language", "id")
        .text("response_format", "json");
    let mut req = client()?.post(format!("{}/audio/transcriptions", openai_base(&base))).multipart(form);
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let v = read_json(req.send().await.map_err(net_err)?).await?;
    v["text"]
        .as_str()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "tidak ada suara yang terbaca".into())
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

fn parse_draft(raw: &str, max_steps: usize) -> Option<Draft> {
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
                .take(max_steps)
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

/// Teks banyak baris (mis. daftar revisi yang di-paste) -> baris pertama judul, sisanya langkah.
fn multiline(text: &str) -> Option<Draft> {
    let lines: Vec<String> = text
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches(|c: char| c == '-' || c == '*' || c == '\u{2022}' || c.is_ascii_digit() || c == '.' || c == ')')
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() < 3 {
        return None;
    }
    let head = heuristic_line(&lines[0]);
    Some(Draft { steps: lines[1..].iter().take(15).cloned().collect(), ..head })
}

pub fn heuristic(text: &str) -> Draft {
    multiline(text).unwrap_or_else(|| heuristic_line(text))
}

pub fn heuristic_tpl(text: &str, tpl: Option<&Template>) -> Draft {
    let mut d = heuristic(text);
    if let Some(t) = tpl {
        if text.lines().filter(|l| !l.trim().is_empty()).count() < 3 {
            d.steps = t.steps.clone();
        }
        d.est_minutes = d.est_minutes.or(t.est_minutes);
    }
    d
}

fn heuristic_line(text: &str) -> Draft {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = joined.as_str();
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
