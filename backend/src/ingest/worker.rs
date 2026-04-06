use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use serde_json::Value;
use anyhow::Result;

use crate::AppState;
use crate::models::{Job, JobStatus, DupMode, SseEvent};
use crate::ingest::{reader, schema::{SchemaTracker, parse_datetime, ColType}, dedup::DedupTracker};

pub async fn run(state: Arc<AppState>, mut job: Job) {
    let start = Instant::now();

    // update status
    {
        let mut jobs = state.jobs.write().await;
        if let Some(j) = jobs.get_mut(&job.id) {
            j.status = JobStatus::Running;
        }
    }

    let result = process(&state, &mut job).await;

    let duration_ms = start.elapsed().as_millis() as u64;
    let (status, error) = match result {
        Ok(_) => (JobStatus::Done, None),
        Err(e) => (JobStatus::Error, Some(e.to_string())),
    };

    // send terminal SSE event
    let sender = {
        let senders = state.senders.read().await;
        senders.get(&job.id).cloned()
    };
    if let Some(tx) = &sender {
        let ev = if error.is_none() {
            SseEvent::Done {
                total_rows: job.rows_inserted,
                duplicates: job.duplicates_found,
                duration_ms,
            }
        } else {
            SseEvent::Error { message: error.clone().unwrap_or_default() }
        };
        let _ = tx.send(ev);
    }

    // update DB
    let _ = sqlx::query(
        "UPDATE _la_jobs SET status=?, error=?, rows_inserted=?, rows_skipped=?, duplicates_found=?, finished_at=NOW() WHERE id=?"
    )
    .bind(serde_json::to_string(&status).unwrap().trim_matches('"').to_string())
    .bind(&error)
    .bind(job.rows_inserted as i64)
    .bind(job.rows_skipped as i64)
    .bind(job.duplicates_found as i64)
    .bind(&job.id)
    .execute(&state.pool)
    .await;

    // update in-memory
    {
        let mut jobs = state.jobs.write().await;
        if let Some(j) = jobs.get_mut(&job.id) {
            j.status = status;
            j.error = error;
            j.rows_inserted = job.rows_inserted;
            j.rows_skipped = job.rows_skipped;
            j.duplicates_found = job.duplicates_found;
        }
    }
}

async fn process(state: &Arc<AppState>, job: &mut Job) -> Result<()> {
    let sender: broadcast::Sender<SseEvent> = {
        let senders = state.senders.read().await;
        senders.get(&job.id).cloned().unwrap()
    };

    let mut schema = SchemaTracker::new();
    schema.ensure_table(&state.pool, &job.table_name).await?;
    schema.load_existing(&state.pool, &job.table_name).await?;

    // Track which cols we've already sent SSE events for (across all files)
    let mut notified_cols: std::collections::HashSet<String> = schema.known.keys().cloned().collect();
    let mut notified_te_cols: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut dedup = DedupTracker::new();
    // Load hashes already stored in the target table for cross-job dedup
    dedup.load_existing(&state.pool, &job.table_name).await?;

    // pre-count total lines across all files
    let mut file_infos: Vec<(String, String, usize)> = Vec::new(); // (file_id, path, lines)
    for file_id in &job.file_ids {
        let row: (String, String) = sqlx::query_as("SELECT original_name, stored_path FROM _la_files WHERE id = ?")
            .bind(file_id)
            .fetch_one(&state.pool)
            .await?;
        let path_for_count = row.1.clone();
        let count = tokio::task::spawn_blocking(move || reader::count_lines(&path_for_count).unwrap_or(0)).await?;
        file_infos.push((file_id.clone(), row.1, count));
        job.total_lines += count;
    }

    for (file_index, (_, stored_path, file_lines)) in file_infos.iter().enumerate() {
        let filename = stored_path.rsplit('/').next().unwrap_or(stored_path).to_string();
        job.current_file_index = file_index + 1;
        job.current_filename = filename.clone();

        let _ = sender.send(SseEvent::FileStart {
            filename: filename.clone(),
            file_index: file_index + 1,
            total_files: job.total_files,
            total_lines: *file_lines,
        });

        // Update in-memory job
        {
            let mut jobs = state.jobs.write().await;
            if let Some(j) = jobs.get_mut(&job.id) {
                j.current_file_index = job.current_file_index;
                j.current_filename = job.current_filename.clone();
                j.total_lines = job.total_lines;
            }
        }

        // Read all lines in a blocking task to avoid holding non-Send iterator across awaits
        let path_clone = stored_path.clone();
        let raw_lines: Vec<String> = tokio::task::spawn_blocking(move || {
            reader::lines(&path_clone)
                .map(|iter| iter.filter_map(|r| r.ok()).collect::<Vec<_>>())
                .unwrap_or_default()
        }).await?;

        let mut local_line = 0usize;

        for raw in raw_lines {
            local_line += 1;
            job.lines_read += 1;

            // duplicate check
            let (is_dup, content_hash) = dedup.check(&raw);
            if is_dup {
                job.duplicates_found += 1;
                match job.dup_mode {
                    DupMode::Skip => {
                        job.rows_skipped += 1;
                        let _ = sender.send(SseEvent::Duplicate {
                            line: local_line,
                            action: "skip".into(),
                        });
                        continue;
                    }
                    DupMode::Warn => {
                        let _ = sender.send(SseEvent::Duplicate {
                            line: local_line,
                            action: "warn".into(),
                        });
                    }
                    DupMode::FlagColumn => {
                        let _ = sender.send(SseEvent::Duplicate {
                            line: local_line,
                            action: "flagged".into(),
                        });
                    }
                }
            }

            // parse JSON
            let obj: serde_json::Map<String, Value> = match serde_json::from_str(&raw) {
                Ok(Value::Object(m)) => m,
                _ => {
                    job.rows_skipped += 1;
                    continue;
                }
            };

            // ensure columns exist
            let mut col_map: Vec<(String, Option<String>, Value)> = Vec::new();
            for (key, val) in &obj {
                let (col, te_col) = schema.ensure_col(&state.pool, &job.table_name, key, val).await?;

                // SSE: notify newly added columns
                if notified_cols.insert(col.clone()) {
                    let sql_type = schema.known.get(&col).map(|t| t.sql().to_string()).unwrap_or_default();
                    if !sql_type.is_empty() {
                        let _ = sender.send(SseEvent::SchemaChange { column: col.clone(), sql_type });
                    }
                }
                if let Some(ref te) = te_col {
                    if notified_te_cols.insert(te.clone()) {
                        let _ = sender.send(SseEvent::TypeErrorColumn {
                            original_column: col.clone(),
                            error_column: te.clone(),
                        });
                    }
                }

                col_map.push((col, te_col, val.clone()));
            }

            // build INSERT
            let is_dup_val: Option<i64> = match job.dup_mode {
                DupMode::FlagColumn if is_dup => Some(1),
                _ => None,
            };

            insert_row(&state.pool, &job.table_name, &job.id, job.lines_read, is_dup_val, &raw, &content_hash, &col_map, &schema).await?;
            job.rows_inserted += 1;

            // send progress every 100 rows or on last line
            if job.rows_inserted % 100 == 0 || local_line == *file_lines {
                let _ = sender.send(SseEvent::Progress {
                    lines_read: job.lines_read,
                    rows_inserted: job.rows_inserted,
                    rows_skipped: job.rows_skipped,
                    total_lines: job.total_lines,
                });
                // sync in-memory
                let mut jobs = state.jobs.write().await;
                if let Some(j) = jobs.get_mut(&job.id) {
                    j.lines_read = job.lines_read;
                    j.rows_inserted = job.rows_inserted;
                    j.rows_skipped = job.rows_skipped;
                    j.duplicates_found = job.duplicates_found;
                }
            }
        }
    }

    Ok(())
}

async fn insert_row(
    pool: &sqlx::MySqlPool,
    table: &str,
    job_id: &str,
    line_no: usize,
    is_dup: Option<i64>,
    raw: &str,
    content_hash: &str,
    col_map: &[(String, Option<String>, Value)],
    schema: &SchemaTracker,
) -> Result<()> {
    let mut cols = vec!["_job_id".to_string(), "_line_no".to_string(), "_is_dup".to_string(), "_content_hash".to_string(), "_raw".to_string()];
    let mut placeholders = vec!["?", "?", "?", "?", "?"];

    for (col, te_col, _) in col_map {
        cols.push(format!("`{}`", col));
        placeholders.push("?");
        if let Some(te) = te_col {
            cols.push(format!("`{}`", te));
            placeholders.push("?");
        }
    }

    let sql = format!(
        "INSERT INTO `{}` ({}) VALUES ({})",
        table,
        cols.join(", "),
        placeholders.join(", ")
    );

    let mut q = sqlx::query(&sql)
        .bind(job_id)
        .bind(line_no as i64)
        .bind(is_dup)
        .bind(content_hash)
        .bind(raw);

    for (col, te_col, val) in col_map {
        let col_type = schema.known.get(col.as_str());
        match (col_type, te_col, val) {
            // type error: null in typed col, raw string in te col
            (_, Some(_), Value::String(s)) => {
                q = q.bind(Option::<String>::None);
                q = q.bind(Some(s.clone()));
            }
            (_, Some(_), _) => {
                q = q.bind(Option::<String>::None);
                q = q.bind(Option::<String>::None);
            }
            // normal
            (Some(ColType::TinyInt), None, Value::Bool(b)) => {
                q = q.bind(Some(*b as i64));
            }
            (Some(ColType::BigInt), None, Value::Number(n)) => {
                q = q.bind(n.as_i64());
            }
            (Some(ColType::Double), None, Value::Number(n)) => {
                q = q.bind(n.as_f64());
            }
            (Some(ColType::DateTime), None, Value::String(s)) => {
                q = q.bind(parse_datetime(s));
            }
            (_, None, Value::String(s)) => {
                q = q.bind(Some(s.clone()));
            }
            (_, None, Value::Null) => {
                q = q.bind(Option::<String>::None);
            }
            (_, None, v) => {
                q = q.bind(Some(v.to_string()));
            }
        }
    }

    q.execute(pool).await?;
    Ok(())
}
