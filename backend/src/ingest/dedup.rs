use std::collections::HashSet;
use sha2::{Sha256, Digest};
use sqlx::MySqlPool;
use anyhow::Result;

pub struct DedupTracker {
    seen: HashSet<[u8; 32]>,
}

impl DedupTracker {
    pub fn new() -> Self {
        Self { seen: HashSet::new() }
    }

    /// Load existing content hashes from the target table into the in-memory set.
    /// This enables cross-job duplicate detection.
    pub async fn load_existing(&mut self, pool: &MySqlPool, table: &str) -> Result<()> {
        // Check if _content_hash column exists first
        let col_exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? AND column_name = '_content_hash'"
        )
        .bind(table)
        .fetch_optional(pool)
        .await?;

        if col_exists.is_none() {
            return Ok(());
        }

        let rows: Vec<(String,)> = sqlx::query_as(
            &format!("SELECT _content_hash FROM `{}` WHERE _content_hash IS NOT NULL", table)
        )
        .fetch_all(pool)
        .await?;

        for (hex,) in rows {
            if let Ok(bytes) = hex_to_hash(&hex) {
                self.seen.insert(bytes);
            }
        }
        Ok(())
    }

    /// Hash the line and check if it's a duplicate. Returns (is_dup, hex_hash).
    pub fn check(&mut self, line: &str) -> (bool, String) {
        let mut hasher = Sha256::new();
        hasher.update(line.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();
        let hex = hex_encode(&hash);
        let is_dup = !self.seen.insert(hash);
        (is_dup, hex)
    }
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_to_hash(hex: &str) -> Result<[u8; 32], ()> {
    if hex.len() != 64 { return Err(()); }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&hex[i*2..i*2+2], 16).map_err(|_| ())?;
    }
    Ok(out)
}
