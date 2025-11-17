use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Algorithm {
    Blake3,
    Sha256,
    Sha512,
    Sha3_256,
    Md5,
}

impl Algorithm {
    pub fn all() -> Vec<Algorithm> {
        vec![Algorithm::Blake3, Algorithm::Sha256, Algorithm::Sha512, Algorithm::Sha3_256, Algorithm::Md5]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Algorithm::Blake3 => "BLAKE3",
            Algorithm::Sha256 => "SHA-256",
            Algorithm::Sha512 => "SHA-512",
            Algorithm::Sha3_256 => "SHA3-256",
            Algorithm::Md5 => "MD5",
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Success,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub id: String,
    pub file_name: String,
    pub file_path: PathBuf,
    pub algorithm: Algorithm,
    pub computed_hash: String,
    pub reference_hash: Option<String>,
    pub status: VerificationStatus,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
