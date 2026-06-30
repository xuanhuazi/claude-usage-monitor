//! Scans `~/.claude/projects/**/*.jsonl`, reading only newly-appended bytes
//! per file (tracked by byte offset) and inserting deduped usage events.

use crate::creds::projects_dir;
use crate::store::Store;
use crate::types::{Event, RawLine};
use chrono::DateTime;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Scan all transcripts; returns the number of new events inserted.
pub fn scan(store: &Store) -> usize {
    let dir = projects_dir();
    if !dir.exists() {
        return 0;
    }
    let mut files = Vec::new();
    collect_jsonl(&dir, &mut files);
    let mut total_new = 0usize;
    for f in files {
        total_new += scan_file(store, &f).unwrap_or(0);
    }
    total_new
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_jsonl(&p, out);
        } else if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn scan_file(store: &Store, path: &Path) -> std::io::Result<usize> {
    let path_str = path.to_string_lossy().to_string();
    let meta = fs::metadata(path)?;
    let size = meta.len() as i64;
    let (mut offset, _prev) = store.get_offset(&path_str);
    if size < offset {
        offset = 0; // file was truncated or rotated
    }
    if size == offset {
        return Ok(0); // no new bytes
    }

    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset as u64))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;

    // Process only up to the final newline; a trailing partial line (still being
    // streamed) is left unconsumed so we re-read it next scan.
    let Some(last_nl) = buf.iter().rposition(|&b| b == b'\n') else {
        return Ok(0);
    };
    let new_offset = offset + last_nl as i64 + 1;
    let text = String::from_utf8_lossy(&buf[..=last_nl]);

    let mut events = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(ev) = parse_line(line) {
            events.push(ev);
        }
    }

    let inserted = store.insert_events(&events).unwrap_or(0);

    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let _ = store.set_offset(&path_str, new_offset, size, mtime_ms);
    Ok(inserted)
}

fn parse_line(line: &str) -> Option<Event> {
    let raw: RawLine = serde_json::from_str(line).ok()?;
    if raw.kind != "assistant" {
        return None;
    }
    let msg = raw.message?;
    let usage = msg.usage?;
    let total = usage.input_tokens
        + usage.output_tokens
        + usage.cache_creation_input_tokens
        + usage.cache_read_input_tokens;
    if total <= 0 {
        return None; // streaming placeholder / non-billable line
    }
    let ts_ms = parse_ts(raw.timestamp.as_deref()?)?;
    // Dedup key: an API response (message.id + requestId) is the same usage even
    // when Claude Code re-logs it across resumed/forked session files. Counting
    // by the per-line `uuid` would multi-count those copies. Fall back to the
    // line uuid only when message.id / requestId are unavailable.
    let uuid = match (msg.id.as_deref(), raw.request_id.as_deref()) {
        (Some(id), Some(req)) => format!("{id}:{req}"),
        _ => raw.uuid?,
    };
    Some(Event {
        uuid,
        ts_ms,
        model: msg.model.unwrap_or_else(|| "unknown".to_string()),
        project: project_name(raw.cwd.as_deref()),
        session_id: raw.session_id.unwrap_or_default(),
        input: usage.input_tokens,
        output: usage.output_tokens,
        cache_creation: usage.cache_creation_input_tokens,
        cache_read: usage.cache_read_input_tokens,
    })
}

fn parse_ts(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn project_name(cwd: Option<&str>) -> String {
    match cwd {
        Some(c) if !c.is_empty() => c
            .rsplit(|ch| ch == '/' || ch == '\\')
            .find(|s| !s.is_empty())
            .unwrap_or(c)
            .to_string(),
        _ => "未知项目".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_key_uses_msgid_and_request_id() {
        let line = r#"{"type":"assistant","uuid":"line-1","requestId":"req-1","timestamp":"2026-06-01T00:00:00.000Z","cwd":"E:\\proj","sessionId":"s1","message":{"id":"msg-1","model":"claude-opus-4-8","usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":2,"cache_read_input_tokens":3}}}"#;
        let e = parse_line(line).unwrap();
        assert_eq!(e.uuid, "msg-1:req-1");
        assert_eq!(e.input, 10);
        assert_eq!(e.output, 5);
        assert_eq!(e.cache_creation, 2);
        assert_eq!(e.cache_read, 3);
        assert_eq!(e.model, "claude-opus-4-8");
        assert_eq!(e.project, "proj");
    }

    #[test]
    fn dedup_key_falls_back_to_line_uuid() {
        let line = r#"{"type":"assistant","uuid":"line-1","timestamp":"2026-06-01T00:00:00.000Z","message":{"model":"m","usage":{"input_tokens":1}}}"#;
        assert_eq!(parse_line(line).unwrap().uuid, "line-1");
    }

    #[test]
    fn skips_non_assistant_lines() {
        let line = r#"{"type":"user","uuid":"u1","timestamp":"2026-06-01T00:00:00.000Z","message":{"role":"user"}}"#;
        assert!(parse_line(line).is_none());
    }

    #[test]
    fn skips_zero_usage_lines() {
        let line = r#"{"type":"assistant","uuid":"x","timestamp":"2026-06-01T00:00:00.000Z","message":{"model":"m","usage":{"input_tokens":0,"output_tokens":0}}}"#;
        assert!(parse_line(line).is_none());
    }
}
