export type DupMode = 'warn' | 'flag_column' | 'skip'

export type JobStatus = 'pending' | 'running' | 'done' | 'error'

export interface Job {
  id: string
  table_name: string
  file_ids: string[]
  dup_mode: DupMode
  status: JobStatus
  error?: string
  created_at: number
  total_files: number
  current_file_index: number
  current_filename: string
  total_lines: number
  lines_read: number
  rows_inserted: number
  rows_skipped: number
  duplicates_found: number
}

export interface UploadResponse {
  file_id: string
  filename: string
  size: number
}

export interface TableInfo {
  name: string
  created_at?: string
}

export type SseEvent =
  | { type: 'file_start'; filename: string; file_index: number; total_files: number; total_lines: number }
  | { type: 'progress'; lines_read: number; rows_inserted: number; rows_skipped: number; total_lines: number }
  | { type: 'schema_change'; column: string; sql_type: string }
  | { type: 'type_error_column'; original_column: string; error_column: string }
  | { type: 'duplicate'; line: number; action: string }
  | { type: 'done'; total_rows: number; duplicates: number; duration_ms: number }
  | { type: 'error'; message: string }
