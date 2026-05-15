use std::sync::Arc;
use std::time::Instant;
use std::collections::{HashMap, HashSet};
use tokio::sync::broadcast;
use serde_json::Value;
use anyhow::Result;
use sqlx::QueryBuilder;

use crate::AppState;
use crate::models::{Job, JobStatus, DupMode, SseEvent};
use crate::ingest::{reader, schema::{SchemaTracker, parse_datetime, ColType}, dedup::DedupTracker};

/// 1回のINSERT文に含める最大行数。
/// MySQL のパラメータ上限(65535)はカラム数に応じて flush_batch 内で自動調整する。
const BATCH_SIZE: usize = 500;

/// バルクINSERT用の1行分データ
struct PendingRow {
    line_no: usize,
    is_dup: Option<i64>,
    content_hash: String,
    raw: String,
    /// ユーザーカラム名 → Option<String> 値（型変換済み）
    col_vals: HashMap<String, Option<String>>,
}

pub async fn run(state: Arc<AppState>, mut job: Job) {
    let start = Instant::now();

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

    let mut notified_cols: HashSet<String> = schema.known.keys().cloned().collect();
    let mut notified_te_cols: HashSet<String> = HashSet::new();

    let mut dedup = DedupTracker::new();
    dedup.load_existing(&state.pool, &job.table_name).await?;

    // ファイルを1回だけ読んでメモリに保存（以前は count_lines + lines で2回読んでいた）
    let mut file_infos: Vec<(String, Vec<String>)> = Vec::new(); // (stored_path, lines)
    for file_id in &job.file_ids {
        let row: (String, String) = sqlx::query_as(
            "SELECT original_name, stored_path FROM _la_files WHERE id = ?",
        )
        .bind(file_id)
        .fetch_one(&state.pool)
        .await?;
        let stored_path = row.1.clone();
        let lines: Vec<String> = tokio::task::spawn_blocking(move || {
            reader::lines(&stored_path)
                .map(|iter| iter.filter_map(|r| r.ok()).collect::<Vec<_>>())
                .unwrap_or_default()
        })
        .await?;
        job.total_lines += lines.len();
        file_infos.push((row.1, lines));
    }

    for (file_index, (stored_path, raw_lines)) in file_infos.iter().enumerate() {
        let filename = stored_path.rsplit('/').next().unwrap_or(stored_path.as_str()).to_string();
        let file_lines = raw_lines.len();
        job.current_file_index = file_index + 1;
        job.current_filename = filename.clone();

        let _ = sender.send(SseEvent::FileStart {
            filename: filename.clone(),
            file_index: file_index + 1,
            total_files: job.total_files,
            total_lines: file_lines,
        });

        {
            let mut jobs = state.jobs.write().await;
            if let Some(j) = jobs.get_mut(&job.id) {
                j.current_file_index = job.current_file_index;
                j.current_filename = job.current_filename.clone();
                j.total_lines = job.total_lines;
            }
        }

        // バルクINSERT用バッファ
        let mut batch: Vec<PendingRow> = Vec::with_capacity(BATCH_SIZE);
        // バッチ内で使われているカラムの挿入順リスト
        let mut ordered_cols: Vec<String> = Vec::new();
        let mut col_set: HashSet<String> = HashSet::new();

        // dup/skip SSEは100行ごとにまとめて送る（毎行送信はSSEオーバーヘッドが大きい）
        let mut pending_dup_warn = 0usize;
        let mut pending_dup_skip = 0usize;
        let mut pending_dup_flag = 0usize;

        let mut local_line = 0usize;

        for raw in raw_lines {
            local_line += 1;
            job.lines_read += 1;

            // 重複チェック
            let (is_dup, content_hash) = dedup.check(raw);
            if is_dup {
                job.duplicates_found += 1;
                match job.dup_mode {
                    DupMode::Skip => {
                        job.rows_skipped += 1;
                        pending_dup_skip += 1;
                        continue;
                    }
                    DupMode::Warn => { pending_dup_warn += 1; }
                    DupMode::FlagColumn => { pending_dup_flag += 1; }
                }
            }

            // JSONパース
            let obj: serde_json::Map<String, Value> = match serde_json::from_str(raw) {
                Ok(Value::Object(m)) => m,
                _ => { job.rows_skipped += 1; continue; }
            };

            // カラム確保 & 値正規化
            let mut col_vals: HashMap<String, Option<String>> = HashMap::with_capacity(obj.len() * 2);
            let mut new_cols_this_row: Vec<String> = Vec::new();

            for (key, val) in &obj {
                let (col, te_col) = schema.ensure_col(&state.pool, &job.table_name, key, val).await?;

                // SSE: 新カラム通知
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

                // このバッチで未登録のカラムを記録
                if !col_set.contains(&col) {
                    new_cols_this_row.push(col.clone());
                }
                if let Some(ref te) = te_col {
                    if !col_set.contains(te) {
                        new_cols_this_row.push(te.clone());
                    }
                }

                // 値をOption<String>に正規化（MySQLが各カラム型へ暗黙変換する）
                let col_type = schema.known.get(&col);
                if let Some(ref te) = te_col {
                    col_vals.insert(col.clone(), None);
                    col_vals.insert(te.clone(), match val {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    });
                } else {
                    col_vals.insert(col.clone(), normalize_value(col_type, val));
                }
            }

            // 新カラムが出てきた場合、既存バッチを先にフラッシュしてカラムセットを安定させる
            if !new_cols_this_row.is_empty() && !batch.is_empty() {
                flush_batch(&state.pool, &job.table_name, &job.id, &schema, &batch, &ordered_cols).await?;
                job.rows_inserted += batch.len();
                batch.clear();
            }

            // ordered_cols に新カラムを追加
            for nc in new_cols_this_row {
                if col_set.insert(nc.clone()) {
                    ordered_cols.push(nc);
                }
            }

            let is_dup_val: Option<i64> = match job.dup_mode {
                DupMode::FlagColumn if is_dup => Some(1),
                _ => None,
            };

            batch.push(PendingRow {
                line_no: job.lines_read,
                is_dup: is_dup_val,
                content_hash,
                raw: raw.clone(),
                col_vals,
            });

            // バッチサイズ到達 → フラッシュ & Progress SSE
            if batch.len() >= BATCH_SIZE {
                flush_batch(&state.pool, &job.table_name, &job.id, &schema, &batch, &ordered_cols).await?;
                job.rows_inserted += batch.len();
                batch.clear();

                flush_dup_events(&sender, &mut pending_dup_skip, &mut pending_dup_warn, &mut pending_dup_flag);

                let _ = sender.send(SseEvent::Progress {
                    lines_read: job.lines_read,
                    rows_inserted: job.rows_inserted,
                    rows_skipped: job.rows_skipped,
                    total_lines: job.total_lines,
                });
                {
                    let mut jobs = state.jobs.write().await;
                    if let Some(j) = jobs.get_mut(&job.id) {
                        j.lines_read = job.lines_read;
                        j.rows_inserted = job.rows_inserted;
                        j.rows_skipped = job.rows_skipped;
                        j.duplicates_found = job.duplicates_found;
                    }
                }
            }
        } // for raw in raw_lines

        // ファイル終端：残りをフラッシュ
        if !batch.is_empty() {
            flush_batch(&state.pool, &job.table_name, &job.id, &schema, &batch, &ordered_cols).await?;
            job.rows_inserted += batch.len();
            batch.clear();
        }

        flush_dup_events(&sender, &mut pending_dup_skip, &mut pending_dup_warn, &mut pending_dup_flag);

        // ファイル終端のProgressイベント
        let _ = sender.send(SseEvent::Progress {
            lines_read: job.lines_read,
            rows_inserted: job.rows_inserted,
            rows_skipped: job.rows_skipped,
            total_lines: job.total_lines,
        });
        {
            let mut jobs = state.jobs.write().await;
            if let Some(j) = jobs.get_mut(&job.id) {
                j.lines_read = job.lines_read;
                j.rows_inserted = job.rows_inserted;
                j.rows_skipped = job.rows_skipped;
                j.duplicates_found = job.duplicates_found;
            }
        }
    } // for file_index

    Ok(())
}

/// 溜めたdup/skipイベントをまとめてSSEに送信
fn flush_dup_events(
    sender: &broadcast::Sender<SseEvent>,
    skip: &mut usize,
    warn: &mut usize,
    flag: &mut usize,
) {
    if *skip > 0 {
        let _ = sender.send(SseEvent::DuplicateBatch { count: *skip, action: "skip".into() });
        *skip = 0;
    }
    if *warn > 0 {
        let _ = sender.send(SseEvent::DuplicateBatch { count: *warn, action: "warn".into() });
        *warn = 0;
    }
    if *flag > 0 {
        let _ = sender.send(SseEvent::DuplicateBatch { count: *flag, action: "flagged".into() });
        *flag = 0;
    }
}

/// JSON値をMySQLカラム型に合わせて Option<String> に正規化。
/// MySQLは文字列からBIGINT/DOUBLE/DATETIME/TINYINTへ暗黙変換する。
fn normalize_value(col_type: Option<&ColType>, val: &Value) -> Option<String> {
    match (col_type, val) {
        (Some(ColType::TinyInt), Value::Bool(b)) => Some(if *b { "1" } else { "0" }.to_string()),
        (Some(ColType::BigInt), Value::Number(n)) => n.as_i64().map(|n| n.to_string()),
        (Some(ColType::Double), Value::Number(n)) => n.as_f64().map(|f| f.to_string()),
        (Some(ColType::DateTime), Value::String(s)) => parse_datetime(s),
        (_, Value::String(s)) => Some(s.clone()),
        (_, Value::Null) => None,
        (_, v) => Some(v.to_string()),
    }
}

/// バルクINSERT（自動拡幅リトライ付き）
async fn flush_batch(
    pool: &sqlx::MySqlPool,
    table: &str,
    job_id: &str,
    schema: &SchemaTracker,
    batch: &[PendingRow],
    ordered_cols: &[String],
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    // MySQLのバインドパラメータ上限(65535)を超えないようチャンク分割
    let params_per_row = 5 + ordered_cols.len();
    let max_per_stmt = if params_per_row > 0 {
        (65535 / params_per_row).max(1)
    } else {
        batch.len()
    };

    for chunk in batch.chunks(max_per_stmt) {
        loop {
            match do_bulk_insert(pool, table, job_id, chunk, ordered_cols).await {
                Ok(()) => break,
                Err(e) => match extract_too_long_column(&e) {
                    Some(col) => {
                        schema.widen_text_column(pool, table, &col).await?;
                        // retry same chunk
                    }
                    None => return Err(e),
                },
            }
        }
    }
    Ok(())
}

async fn do_bulk_insert(
    pool: &sqlx::MySqlPool,
    table: &str,
    job_id: &str,
    batch: &[PendingRow],
    ordered_cols: &[String],
) -> Result<()> {
    let col_list = {
        let mut parts = vec![
            "_job_id".to_string(),
            "_line_no".to_string(),
            "_is_dup".to_string(),
            "_content_hash".to_string(),
            "_raw".to_string(),
        ];
        parts.extend(ordered_cols.iter().map(|c| format!("`{}`", c)));
        parts.join(", ")
    };

    let mut qb = QueryBuilder::<sqlx::MySql>::new(
        format!("INSERT INTO `{}` ({}) ", table, col_list),
    );

    qb.push_values(batch.iter(), |mut b, row| {
        b.push_bind(job_id);
        b.push_bind(row.line_no as i64);
        b.push_bind(row.is_dup);
        b.push_bind(row.content_hash.as_str());
        b.push_bind(row.raw.as_str());
        for col in ordered_cols {
            // col_valsにない場合はNULL（この行にそのキーが存在しなかった）
            b.push_bind(row.col_vals.get(col).cloned().flatten());
        }
    });

    qb.build().execute(pool).await?;
    Ok(())
}

/// MySQL error 1406 ("Data too long for column 'X'") からカラム名を抽出する。
fn extract_too_long_column(err: &anyhow::Error) -> Option<String> {
    for cause in err.chain() {
        if let Some(db_err) = cause.downcast_ref::<sqlx::Error>() {
            if let sqlx::Error::Database(dbe) = db_err {
                if dbe.code().as_deref() != Some("22001")
                    && !dbe.message().to_lowercase().contains("data too long")
                {
                    continue;
                }
                let msg = dbe.message();
                if let Some(start) = msg.find('\'') {
                    let rest = &msg[start + 1..];
                    if let Some(end) = rest.find('\'') {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
    }
    let s = err.to_string();
    if !s.to_lowercase().contains("data too long") {
        return None;
    }
    let start = s.find('\'')?;
    let rest = &s[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}
