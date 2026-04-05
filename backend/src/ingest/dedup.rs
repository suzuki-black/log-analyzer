use std::collections::HashSet;
use sha2::{Sha256, Digest};

pub struct DedupTracker {
    seen: HashSet<[u8; 32]>,
}

impl DedupTracker {
    pub fn new() -> Self {
        Self { seen: HashSet::new() }
    }

    /// Returns true if this line is a duplicate
    pub fn check(&mut self, line: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(line.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();
        !self.seen.insert(hash)
    }
}
