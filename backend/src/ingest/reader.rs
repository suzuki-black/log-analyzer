use std::io::{BufRead, BufReader, Read};
use flate2::read::GzDecoder;
use anyhow::Result;

/// Count non-empty lines in a file (supports .gz)
pub fn count_lines(path: &str) -> Result<usize> {
    let file = std::fs::File::open(path)?;
    let reader: Box<dyn Read> = if path.ends_with(".gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let count = BufReader::new(reader)
        .lines()
        .filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false))
        .count();
    Ok(count)
}

/// Iterate non-empty lines from a file (supports .gz)
pub fn lines(path: &str) -> Result<impl Iterator<Item = Result<String>>> {
    let file = std::fs::File::open(path)?;
    let reader: Box<dyn Read> = if path.ends_with(".gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let iter = BufReader::new(reader)
        .lines()
        .filter_map(|r| match r {
            Ok(s) if s.trim().is_empty() => None,
            Ok(s) => Some(Ok(s)),
            Err(e) => Some(Err(anyhow::anyhow!(e))),
        });
    Ok(iter)
}
