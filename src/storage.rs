use crate::models::VerificationRecord;
use serde_json;
use std::path::PathBuf;
use std::fs;
use anyhow::Result;

const DB_FILE: &str = "verifications.json";

pub fn load_all() -> Vec<VerificationRecord> {
    let path = PathBuf::from(DB_FILE);
    if !path.exists() { return Vec::new(); }
    let s = fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&s).unwrap_or_default()
}

pub fn save_all(records: &[VerificationRecord]) -> Result<()> {
    let s = serde_json::to_string_pretty(records)?;
    fs::write(DB_FILE, s)?;
    Ok(())
}
