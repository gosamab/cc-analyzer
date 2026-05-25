use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::db::Db;
use crate::pricing::cost_usd;

pub fn projects_root() -> PathBuf {
    dirs::home_dir()
        .expect("no home dir")
        .join(".claude")
        .join("projects")
}

/// Walk projects_root, ingest any new bytes from each JSONL file.
/// Returns the number of new rows inserted.
pub fn refresh(db: &Db) -> Result<usize> {
    let root = projects_root();
    if !root.exists() {
        return Ok(0);
    }
    let mut inserted = 0;
    for entry in walkdir::WalkDir::new(&root)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        inserted += ingest_file(db, p).unwrap_or(0);
    }
    Ok(inserted)
}

fn ingest_file(db: &Db, path: &Path) -> Result<usize> {
    let project = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();
    let path_str = path.to_string_lossy().to_string();
    let start_off = db.get_offset(&path_str)?;
    let mut file = File::open(path)?;
    let end = file.metadata()?.len();
    if end <= start_off {
        return Ok(0);
    }
    file.seek(SeekFrom::Start(start_off))?;
    let reader = BufReader::new(file);
    let tx = db.conn.unchecked_transaction()?;
    let mut count = 0usize;
    let mut bytes_consumed: u64 = start_off;
    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };
        bytes_consumed += line.len() as u64 + 1; // \n
        let Ok(d): Result<Value, _> = serde_json::from_str(&line) else { continue };
        let kind = d.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "ai-title" {
            let sid = d.get("sessionId").and_then(|v| v.as_str()).unwrap_or(&session_id);
            if let Some(title) = d.get("aiTitle").and_then(|v| v.as_str()) {
                db.upsert_title(sid, title).ok();
            }
            continue;
        }
        if kind == "user" {
            if let Some(cmd) = extract_slash_command(&d) {
                let ts = d.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();
                db.insert_slash_command(&session_id, &project, &ts, &cmd).ok();
            }
            continue;
        }
        if kind != "assistant" {
            continue;
        }
        let ts = d.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let msg = d.get("message").cloned().unwrap_or(Value::Null);
        let model = msg
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let u = msg.get("usage").cloned().unwrap_or(Value::Null);
        let inp = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let out = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let cw = u
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cr = u
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cost = cost_usd(&model, inp, out, cw, cr);

        // collect tool uses as compact JSON: [{name,file_path?,cmd?}]
        let mut tools: Vec<Value> = Vec::new();
        if let Some(content) = msg.get("content").and_then(|v| v.as_array()) {
            for b in content {
                if b.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    let name = b.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let input = b.get("input").cloned().unwrap_or(Value::Null);
                    let mut entry = serde_json::json!({ "name": name });
                    if name == "Read" {
                        if let Some(fp) = input.get("file_path").and_then(|v| v.as_str()) {
                            entry["file_path"] = Value::String(fp.to_string());
                        }
                    } else if name == "Bash" {
                        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                            let truncated: String = cmd.chars().take(500).collect();
                            entry["cmd"] = Value::String(truncated);
                        }
                    } else if name == "Skill" {
                        if let Some(s) = input.get("skill").and_then(|v| v.as_str()) {
                            entry["skill"] = Value::String(s.to_string());
                        }
                    }
                    tools.push(entry);
                }
            }
        }
        let tools_json = serde_json::to_string(&tools)?;

        tx.execute(
            "INSERT OR IGNORE INTO messages
             (session_id, project, ts, model, input_tok, output_tok, cache_w_tok, cache_r_tok, cost_usd, tools_json)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                session_id,
                project,
                ts,
                model,
                inp as i64,
                out as i64,
                cw as i64,
                cr as i64,
                cost,
                tools_json
            ],
        )?;
        count += 1;
    }
    tx.commit()?;
    db.set_offset(&path_str, bytes_consumed.min(end))?;
    Ok(count)
}

/// Pull a leading `/foo` slash command out of a `type:"user"` line, if present.
/// User content can be either a plain string or a list of content blocks; the
/// marker we look for is `<command-name>/foo</command-name>`. Returns the
/// command including the leading `/`, lowercased and trimmed.
fn extract_slash_command(d: &Value) -> Option<String> {
    let msg = d.get("message")?;
    let content = msg.get("content")?;
    let scan = |s: &str| -> Option<String> {
        let start = s.find("<command-name>")?;
        let rest = &s[start + "<command-name>".len()..];
        let end = rest.find("</command-name>")?;
        let raw = rest[..end].trim();
        if raw.is_empty() {
            return None;
        }
        let cmd = if raw.starts_with('/') {
            raw.to_string()
        } else {
            format!("/{raw}")
        };
        Some(cmd)
    };
    match content {
        Value::String(s) => scan(s),
        Value::Array(blocks) => blocks.iter().find_map(|b| {
            let t = b.get("type").and_then(|v| v.as_str())?;
            if t != "text" {
                return None;
            }
            let txt = b.get("text").and_then(|v| v.as_str())?;
            scan(txt)
        }),
        _ => None,
    }
}
