use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DupMode {
    Warn,
    FlagColumn,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub table_name: String,
    pub file_ids: Vec<String>,
    pub dup_mode: DupMode,
    pub status: JobStatus,
    pub error: Option<String>,
    pub created_at: u64,
    // progress
    pub total_files: usize,
    pub current_file_index: usize,
    pub current_filename: String,
    pub total_lines: usize,
    pub lines_read: usize,
    pub rows_inserted: usize,
    pub rows_skipped: usize,
    pub duplicates_found: usize,
}

impl Job {
    pub fn new(id: String, table_name: String, file_ids: Vec<String>, dup_mode: DupMode) -> Self {
        let total_files = file_ids.len();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            id,
            table_name,
            file_ids,
            dup_mode,
            status: JobStatus::Pending,
            error: None,
            created_at: now,
            total_files,
            current_file_index: 0,
            current_filename: String::new(),
            total_lines: 0,
            lines_read: 0,
            rows_inserted: 0,
            rows_skipped: 0,
            duplicates_found: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    FileStart {
        filename: String,
        file_index: usize,
        total_files: usize,
        total_lines: usize,
    },
    Progress {
        lines_read: usize,
        rows_inserted: usize,
        rows_skipped: usize,
        total_lines: usize,
    },
    SchemaChange {
        column: String,
        sql_type: String,
    },
    TypeErrorColumn {
        original_column: String,
        error_column: String,
    },
    Duplicate {
        line: usize,
        action: String,
    },
    Done {
        total_rows: usize,
        duplicates: usize,
        duration_ms: u64,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub table_name: String,
    pub file_ids: Vec<String>,
    pub dup_mode: DupMode,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub file_id: String,
    pub filename: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub created_at: Option<String>,
}
