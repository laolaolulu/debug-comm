use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 1000;

static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReceiveLogRecord {
    pub id: String,
    pub received_at: u64,
    pub workflow_id: String,
    pub step_id: String,
    pub source_step_id: String,
    pub byte_len: usize,
    pub msg: Value,
}

pub fn create_record(
    workflow_id: impl Into<String>,
    step_id: impl Into<String>,
    source_step_id: impl Into<String>,
    msg: Value,
) -> ReceiveLogRecord {
    let received_at = current_time_millis();
    let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let byte_len = message_byte_len(&msg);

    ReceiveLogRecord {
        id: format!("{received_at:013}-{sequence:010}"),
        received_at,
        workflow_id: workflow_id.into(),
        step_id: step_id.into(),
        source_step_id: source_step_id.into(),
        byte_len,
        msg,
    }
}

pub fn append_record(base_dir: &Path, record: &ReceiveLogRecord) -> Result<(), String> {
    let path = log_file_path(base_dir, &record.workflow_id, &record.step_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    let line = serde_json::to_string(record).map_err(|err| err.to_string())?;
    writeln!(file, "{line}").map_err(|err| err.to_string())
}

pub fn read_records(
    base_dir: &Path,
    workflow_id: &str,
    step_id: &str,
    before: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<ReceiveLogRecord>, String> {
    let path = log_file_path(base_dir, workflow_id, step_id);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let file = File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut records = Vec::<ReceiveLogRecord>::new();

    for line in reader.lines() {
        let line = line.map_err(|err| err.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ReceiveLogRecord>(&line) {
            Ok(record) => records.push(record),
            Err(err) => eprintln!("failed to parse receive log line: {err}"),
        }
    }

    let end = before
        .and_then(|cursor| records.iter().position(|record| record.id == cursor))
        .unwrap_or(records.len());
    let start = end.saturating_sub(limit);

    Ok(records[start..end].to_vec())
}

pub fn clear_records(base_dir: &Path, workflow_id: &str, step_id: &str) -> Result<(), String> {
    let path = log_file_path(base_dir, workflow_id, step_id);
    if path.exists() {
        fs::remove_file(path).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn log_file_path(base_dir: &Path, workflow_id: &str, step_id: &str) -> PathBuf {
    base_dir
        .join("receive-logs")
        .join(safe_segment(workflow_id))
        .join(format!("{}.jsonl", safe_segment(step_id)))
}

fn safe_segment(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn message_byte_len(value: &Value) -> usize {
    match value {
        Value::String(text) => text.as_bytes().len(),
        Value::Array(items) => items
            .iter()
            .filter(|item| {
                item.as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .is_some()
            })
            .count(),
        _ => serde_json::to_vec(value)
            .map(|bytes| bytes.len())
            .unwrap_or_default(),
    }
}
