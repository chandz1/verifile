use std::fs::File;
use anyhow::Result;
use std::path::Path;
use crate::hashers;
use crate::models::Algorithm;
use std::io::BufReader;

/// Compute hash of the file at path using streaming read.
/// This is synchronous; call it inside a spawned thread/task to keep UI responsive.
pub fn compute_file_hash(path: &Path, algo: &Algorithm) -> Result<String> {
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    let hex = hashers::compute_hash_for_reader(reader, algo)?;
    Ok(hex)
}
