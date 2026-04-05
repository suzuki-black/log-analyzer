pub struct Config {
    pub database_url: String,
    pub upload_dir: String,
    pub table_suffix: String,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL required"),
            upload_dir: std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "/uploads".into()),
            table_suffix: std::env::var("TABLE_SUFFIX").unwrap_or_else(|_| "_la".into()),
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT").unwrap_or_else(|_| "8080".into()).parse().unwrap_or(8080),
        }
    }
}
