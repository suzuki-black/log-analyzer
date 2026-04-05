import { useEffect, useState } from 'react'
import { useParams, Link } from 'react-router-dom'
import { getJob } from '../api/client'
import { useSSE } from '../hooks/useSSE'
import type { Job, SseEvent } from '../types'
import styles from './JobPage.module.css'

export default function JobPage() {
  const { id } = useParams<{ id: string }>()
  const [job, setJob] = useState<Job | null>(null)
  const { events } = useSSE(id ?? null)

  // Poll job status on mount + when done
  useEffect(() => {
    if (!id) return
    getJob(id).then(setJob).catch(() => {})
    const iv = setInterval(() => {
      getJob(id).then(j => {
        setJob(j)
        if (j.status === 'done' || j.status === 'error') clearInterval(iv)
      }).catch(() => {})
    }, 1000)
    return () => clearInterval(iv)
  }, [id])

  // Derive progress from SSE events
  const progress = deriveProgress(events, job)

  return (
    <div className={styles.page}>
      <div className={styles.topBar}>
        <Link to="/">← ホームへ戻る</Link>
        {job && <span className={styles.tableLabel}>{job.table_name}</span>}
      </div>

      {job && (
        <>
          <div className={styles.statusRow}>
            <StatusBadge status={job.status} />
            {job.status === 'error' && job.error && (
              <span className={styles.errorMsg}>{job.error}</span>
            )}
          </div>

          {/* Progress modal panel */}
          <div className={`card ${styles.progressCard}`}>
            <h2 className={styles.cardTitle}>取り込み進捗</h2>

            {/* File progress */}
            <ProgressSection
              label={`ファイル: ${progress.fileIndex} / ${job.total_files}`}
              sub={progress.currentFile || '—'}
              value={job.total_files > 0 ? progress.fileIndex / job.total_files : 0}
            />

            {/* Record progress */}
            <ProgressSection
              label={`レコード: ${progress.linesRead.toLocaleString()} / ${progress.totalLines.toLocaleString()}`}
              sub={`挿入: ${progress.rowsInserted.toLocaleString()}　スキップ: ${progress.rowsSkipped.toLocaleString()}　重複: ${job.duplicates_found.toLocaleString()}`}
              value={progress.totalLines > 0 ? progress.linesRead / progress.totalLines : 0}
            />

            {/* Event log */}
            <div className={styles.eventLog}>
              {events.length === 0 && <p className={styles.emptyLog}>イベント待機中...</p>}
              {[...events].reverse().map((ev, i) => (
                <EventRow key={i} ev={ev} />
              ))}
            </div>
          </div>
        </>
      )}

      {!job && <p className={styles.loading}>読み込み中...</p>}
    </div>
  )
}

function deriveProgress(events: SseEvent[], job: Job | null) {
  let fileIndex = job?.current_file_index ?? 0
  let currentFile = job?.current_filename ?? ''
  let linesRead = job?.lines_read ?? 0
  let rowsInserted = job?.rows_inserted ?? 0
  let rowsSkipped = job?.rows_skipped ?? 0
  let totalLines = job?.total_lines ?? 0

  for (const ev of events) {
    if (ev.type === 'file_start') {
      fileIndex = ev.file_index
      currentFile = ev.filename
      if (ev.total_lines > 0) totalLines = ev.total_lines
    }
    if (ev.type === 'progress') {
      linesRead = Math.max(linesRead, ev.lines_read)
      rowsInserted = Math.max(rowsInserted, ev.rows_inserted)
      rowsSkipped = Math.max(rowsSkipped, ev.rows_skipped)
      totalLines = Math.max(totalLines, ev.total_lines)
    }
    if (ev.type === 'done') {
      rowsInserted = ev.total_rows
      linesRead = totalLines
    }
  }
  return { fileIndex, currentFile, linesRead, rowsInserted, rowsSkipped, totalLines }
}

function ProgressSection({ label, sub, value }: { label: string; sub: string; value: number }) {
  const pct = Math.min(100, Math.round(value * 100))
  return (
    <div className={styles.progressSection}>
      <div className={styles.progressLabel}>
        <span>{label}</span>
        <span>{pct}%</span>
      </div>
      <div className={styles.progressTrack}>
        <div className={styles.progressFill} style={{ width: `${pct}%` }} />
      </div>
      <div className={styles.progressSub}>{sub}</div>
    </div>
  )
}

function EventRow({ ev }: { ev: SseEvent }) {
  const [icon, color, text] = formatEvent(ev)
  return (
    <div className={styles.eventRow} style={{ borderLeftColor: color }}>
      <span className={styles.eventIcon}>{icon}</span>
      <span style={{ color }}>{text}</span>
    </div>
  )
}

function formatEvent(ev: SseEvent): [string, string, string] {
  switch (ev.type) {
    case 'file_start':
      return ['📂', '#90cdf4', `ファイル開始: ${ev.filename} (${ev.file_index}/${ev.total_files}, 約${ev.total_lines.toLocaleString()}行)`]
    case 'progress':
      return ['📊', '#68d391', `進捗: ${ev.lines_read.toLocaleString()}行読込 / 挿入${ev.rows_inserted.toLocaleString()} / スキップ${ev.rows_skipped.toLocaleString()}`]
    case 'schema_change':
      return ['➕', '#b794f4', `新規カラム追加: ${ev.column} (${ev.sql_type})`]
    case 'type_error_column':
      return ['⚠', '#f6ad55', `型エラーカラム追加: ${ev.error_column} (元: ${ev.original_column})`]
    case 'duplicate':
      return ['🔁', '#fc8181', `重複検出: ${ev.line}行目 → ${ev.action}`]
    case 'done':
      return ['✅', '#68d391', `完了: ${ev.total_rows.toLocaleString()}行挿入 / 重複${ev.duplicates.toLocaleString()}件 / ${ev.duration_ms}ms`]
    case 'error':
      return ['❌', '#fc8181', `エラー: ${ev.message}`]
    default:
      return ['•', '#718096', JSON.stringify(ev)]
  }
}

function StatusBadge({ status }: { status: string }) {
  const map: Record<string, [string, string]> = {
    pending: ['#718096', 'PENDING'],
    running: ['#3182ce', 'RUNNING'],
    done: ['#38a169', 'DONE'],
    error: ['#e53e3e', 'ERROR'],
  }
  const [color, label] = map[status] ?? ['#718096', status]
  return <span className="tag" style={{ background: color + '33', color, fontSize: 13 }}>{label}</span>
}
