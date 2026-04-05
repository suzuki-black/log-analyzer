use std::collections::HashMap;
use sqlx::MySqlPool;
use anyhow::Result;
use serde_json::Value;
use chrono::DateTime;

/// MySQL column type we track
#[derive(Debug, Clone, PartialEq)]
pub enum ColType {
    TinyInt,
    BigInt,
    Double,
    DateTime,
    Text,
}

impl ColType {
    pub fn sql(&self) -> &'static str {
        match self {
            ColType::TinyInt => "TINYINT(1)",
            ColType::BigInt => "BIGINT",
            ColType::Double => "DOUBLE",
            ColType::DateTime => "DATETIME",
            ColType::Text => "TEXT",
        }
    }
}

/// Infer the ColType for a JSON value
pub fn infer_type(v: &Value) -> ColType {
    match v {
        Value::Null => ColType::Text,
        Value::Bool(_) => ColType::TinyInt,
        Value::Number(n) => {
            if n.is_f64() { ColType::Double } else { ColType::BigInt }
        }
        Value::String(s) => {
            if looks_like_datetime(s) { ColType::DateTime } else { ColType::Text }
        }
        _ => ColType::Text,
    }
}

/// Check if a string looks like a datetime
pub fn looks_like_datetime(s: &str) -> bool {
    // ISO 8601 with time component
    if DateTime::parse_from_rfc3339(s).is_ok() {
        return true;
    }
    // YYYY-MM-DD HH:MM:SS
    if chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_ok() {
        return true;
    }
    // YYYY-MM-DDTHH:MM:SS (no tz)
    if chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").is_ok() {
        return true;
    }
    // YYYY-MM-DDTHH:MM:SS.fff
    if chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").is_ok() {
        return true;
    }
    false
}

/// Parse a string to MySQL DATETIME string (UTC), returns None if unparseable
pub fn parse_datetime(s: &str) -> Option<String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    None
}

/// Sanitize a JSON key to a safe SQL column name
pub fn sanitize_col(key: &str) -> String {
    let s: String = key.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let s = s.to_lowercase();
    // prefix reserved words
    let reserved = ["select","from","where","insert","update","delete","create","drop",
                    "alter","table","index","order","group","by","limit","offset",
                    "join","left","right","inner","outer","on","as","and","or","not",
                    "null","true","false","primary","key","unique","foreign","references",
                    "default","values","set","into","is","in","like","between","case","when",
                    "then","else","end","exists","having","union","all","distinct","count",
                    "sum","avg","max","min","if","schema","database","show","use","id"];
    if reserved.contains(&s.as_str()) || s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("k_{}", s)
    } else {
        s
    }
}

pub struct SchemaTracker {
    /// col_name -> ColType
    pub known: HashMap<String, ColType>,
    /// col_name -> type_error companion col name
    pub type_error_cols: HashMap<String, String>,
}

impl SchemaTracker {
    pub fn new() -> Self {
        Self {
            known: HashMap::new(),
            type_error_cols: HashMap::new(),
        }
    }

    /// Ensure table exists with system columns
    pub async fn ensure_table(&self, pool: &MySqlPool, table: &str) -> Result<()> {
        sqlx::query(&format!(
            r#"CREATE TABLE IF NOT EXISTS `{table}` (
                _id     BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                _job_id VARCHAR(36) NOT NULL,
                _line_no BIGINT NOT NULL,
                _is_dup  TINYINT(1) DEFAULT NULL,
                _raw     MEDIUMTEXT NOT NULL
            )"#
        ))
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Load existing columns from information_schema
    pub async fn load_existing(&mut self, pool: &MySqlPool, table: &str) -> Result<()> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT column_name, data_type FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ordinal_position"
        )
        .bind(table)
        .fetch_all(pool)
        .await?;

        for (col, dtype) in rows {
            if col.starts_with('_') { continue; }
            let ct = match dtype.to_uppercase().as_str() {
                "TINYINT" => ColType::TinyInt,
                "BIGINT" => ColType::BigInt,
                "DOUBLE" => ColType::Double,
                "DATETIME" => ColType::DateTime,
                _ => ColType::Text,
            };
            self.known.insert(col, ct);
        }
        Ok(())
    }

    /// Ensure a column exists; if type conflicts, ensure type_error companion col too.
    /// Returns (col_name, type_error_col_name_if_conflict)
    pub async fn ensure_col(
        &mut self,
        pool: &MySqlPool,
        table: &str,
        key: &str,
        val: &Value,
    ) -> Result<(String, Option<String>)> {
        let col = sanitize_col(key);
        let inferred = infer_type(val);

        if let Some(existing) = self.known.get(&col) {
            if *existing != inferred && !matches!(val, Value::Null) {
                // type mismatch: ensure _te_ companion col
                let te_col = format!("_te_{}", col);
                if !self.type_error_cols.contains_key(&col) {
                    self.add_column(pool, table, &te_col, "TEXT").await?;
                    self.type_error_cols.insert(col.clone(), te_col.clone());
                }
                return Ok((col.clone(), Some(self.type_error_cols[&col].clone())));
            }
            return Ok((col, None));
        }

        // new column
        self.add_column(pool, table, &col, inferred.sql()).await?;
        self.known.insert(col.clone(), inferred);
        Ok((col, None))
    }

    async fn add_column(&self, pool: &MySqlPool, table: &str, col: &str, sql_type: &str) -> Result<()> {
        // MySQL 8 does NOT support IF NOT EXISTS for ALTER TABLE ADD COLUMN (MariaDB only)
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?"
        )
        .bind(table)
        .bind(col)
        .fetch_optional(pool)
        .await?;

        if exists.is_none() {
            sqlx::query(&format!(
                "ALTER TABLE `{table}` ADD COLUMN `{col}` {sql_type} DEFAULT NULL"
            ))
            .execute(pool)
            .await?;
        }
        Ok(())
    }
}
