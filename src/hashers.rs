use crate::models::Algorithm;
use anyhow::Result;
use std::io::Read;

pub fn compute_hash_for_reader<R: Read>(mut reader: R, algorithm: &Algorithm) -> Result<String> {
    match algorithm {
        Algorithm::Blake3 => {
            let mut hasher = blake3::Hasher::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            Ok(hasher.finalize().to_hex().to_string())
        }
        Algorithm::Sha256 => {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        }
        Algorithm::Sha512 => {
            use sha2::{Digest, Sha512};
            let mut hasher = Sha512::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        }
        Algorithm::Sha3_256 => {
            use sha3::{Digest, Sha3_256};
            let mut hasher = Sha3_256::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        }
            Algorithm::Md5 => {
            use md5::Context;
            let mut ctx = Context::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 { break; }
                ctx.consume(&buf[..n]);
            }
            Ok(format!("{:x}", ctx.finalize()))
        }
    }
}
