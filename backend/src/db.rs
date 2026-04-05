use sqlx::MySqlPool;

pub async fn create_pool(url: &str) -> MySqlPool {
    MySqlPool::connect(url).await.expect("Failed to connect to MySQL")
}

pub async fn init(pool: &MySqlPool) {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS _la_files (
            id VARCHAR(36) PRIMARY KEY,
            original_name VARCHAR(512) NOT NULL,
            stored_path VARCHAR(1024) NOT NULL,
            size BIGINT NOT NULL,
            uploaded_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"#,
    )
    .execute(pool)
    .await
    .expect("Failed to create _la_files table");

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS _la_jobs (
            id VARCHAR(36) PRIMARY KEY,
            table_name VARCHAR(256) NOT NULL,
            dup_mode VARCHAR(32) NOT NULL,
            status VARCHAR(32) NOT NULL,
            error TEXT,
            total_files INT DEFAULT 0,
            rows_inserted BIGINT DEFAULT 0,
            rows_skipped BIGINT DEFAULT 0,
            duplicates_found BIGINT DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            finished_at DATETIME
        )"#,
    )
    .execute(pool)
    .await
    .expect("Failed to create _la_jobs table");
}
