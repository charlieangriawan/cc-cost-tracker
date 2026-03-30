use crate::cost::{calculate_cost, normalize_model};
use crate::models::{RawEvent, UsageRecord};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Parse a JSONL file and return deduplicated UsageRecords.
///
/// Claude Code streams each API response as multiple JSONL lines sharing the
/// same `requestId`, each carrying the **cumulative** token totals so far.
/// We keep only the LAST event per requestId (the final, complete totals).
///
/// `seen` is a global set shared across all files — the same requestId can
/// appear in both a main session file and its subagent file; `seen` prevents
/// double-counting those cross-file duplicates.
pub fn parse_jsonl_file(path: &Path, seen: &mut HashMap<String, RawEvent>) -> Vec<UsageRecord> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Cannot open {:?}: {}", path, e);
            return vec![];
        }
    };

    // Collect last event per requestId within this file first
    let mut by_request: HashMap<String, RawEvent> = HashMap::new();

    for line in BufReader::new(file).lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let event: RawEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if event.event_type.as_deref() != Some("assistant") {
            continue;
        }

        if event.message.as_ref().and_then(|m| m.usage.as_ref()).is_none() {
            continue;
        }

        let model = event.message.as_ref().and_then(|m| m.model.as_deref()).unwrap_or("");
        if model == "<synthetic>" || model.is_empty() {
            continue;
        }

        let request_id = event
            .request_id
            .clone()
            .or_else(|| event.message.as_ref().and_then(|m| m.id.clone()))
            .unwrap_or_else(|| {
                event.timestamp.clone().unwrap_or_else(|| format!("anon-{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)))
            });

        // Last line per requestId wins (streaming cumulative totals)
        by_request.insert(request_id, event);
    }

    // Merge into global seen map; skip any requestId already recorded
    let mut new_records = Vec::new();
    for (rid, event) in by_request {
        if !seen.contains_key(&rid) {
            seen.insert(rid, event.clone());
            if let Some(record) = event_to_record(event) {
                new_records.push(record);
            }
        }
    }
    new_records
}

fn event_to_record(event: RawEvent) -> Option<UsageRecord> {
    let message = event.message?;
    let usage = message.usage?;

    let raw_model = message.model.as_deref().unwrap_or("unknown");
    let model = normalize_model(raw_model);

    let input_tokens       = usage.input_tokens.unwrap_or(0);
    let output_tokens      = usage.output_tokens.unwrap_or(0);
    let cache_write_tokens = usage.cache_creation_input_tokens.unwrap_or(0);
    let cache_read_tokens  = usage.cache_read_input_tokens.unwrap_or(0);

    let (cost_input, cost_output, cost_cache_write, cost_cache_read) =
        calculate_cost(input_tokens, output_tokens, cache_write_tokens, cache_read_tokens, &model);

    let total_cost = cost_input + cost_output + cost_cache_write + cost_cache_read;

    let timestamp = event
        .timestamp
        .as_deref()
        .and_then(|t| t.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);

    let project = event.cwd.as_deref().map(extract_project_name).unwrap_or_else(|| "unknown".into());

    let request_id = event
        .request_id
        .or_else(|| message.id)
        .unwrap_or_else(|| format!("anon-{}", timestamp.timestamp_nanos_opt().unwrap_or(0)));

    Some(UsageRecord {
        request_id,
        session_id: event.session_id.unwrap_or_default(),
        project,
        model,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
        cost_input,
        cost_output,
        cost_cache_write,
        cost_cache_read,
        total_cost,
        timestamp,
    })
}

/// "/Users/alice/Development/org/project" → "org/project"
fn extract_project_name(cwd: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let rel = if !home.is_empty() && cwd.starts_with(&home) {
        cwd[home.len()..].trim_start_matches('/')
    } else {
        cwd.trim_start_matches('/')
    };

    let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => "unknown".into(),
        1 => parts[0].into(),
        n => format!("{}/{}", parts[n - 2], parts[n - 1]),
    }
}

/// Scan all JSONL files under ~/.claude/projects/ including subagent files.
/// Uses a global `seen` map to deduplicate requestIds across files —
/// the same requestId can appear in both a main session file and its
/// subagent file; the global map prevents double-counting.
pub fn scan_all_records() -> Vec<UsageRecord> {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return vec![],
    };
    let projects_dir = std::path::PathBuf::from(&home).join(".claude").join("projects");

    let mut all = Vec::new();
    let mut seen: HashMap<String, RawEvent> = HashMap::new();

    let project_dirs = match std::fs::read_dir(&projects_dir) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Cannot read {:?}: {}", projects_dir, e);
            return vec![];
        }
    };

    for proj_entry in project_dirs.flatten() {
        let proj_path = proj_entry.path();
        if !proj_path.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(&proj_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "jsonl") {
                all.extend(parse_jsonl_file(&p, &mut seen));
            } else if p.is_dir() {
                let subagents = p.join("subagents");
                if subagents.is_dir() {
                    for agent_entry in std::fs::read_dir(&subagents).into_iter().flatten().flatten() {
                        let ap = agent_entry.path();
                        if ap.extension().map_or(false, |e| e == "jsonl") {
                            all.extend(parse_jsonl_file(&ap, &mut seen));
                        }
                    }
                }
            }
        }
    }

    all
}
