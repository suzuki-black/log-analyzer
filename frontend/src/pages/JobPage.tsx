import { useEffect, useRef, useState } from 'react'
import { useParams, Link, useNavigate } from 'react-router-dom'
import { getJob } from '../api/client'
import { useSSE } from '../hooks/useSSE'
import { useI18n } from '../i18n'
import type { Job, SseEvent } from '../types'
import styles from './JobPage.module.css'

export default function JobPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const { t } = useI18n()
  const [job, setJob] = useState<Job | null>(null)
  const [doneModal, setDoneModal] = useState<{ status: 'done' | 'error'; rows: number; dups: number; ms: number; error?: string } | null>(null)
  const { events } = useSSE(id ?? null)
  const notifiedRef = useRef(false)

  // Poll job status; show modal on first transition to done/error
  useEffect(() => {
    if (!id) return
    getJob(id).then(setJob).catch(() => {})
    const iv = setInterval(() => {
      getJob(id).then(j => {
        setJob(j)
        if ((j.status === 'done' || j.status === 'error') && !notifiedRef.current) {
          notifiedRef.current = true
          clearInterval(iv)
          setDoneModal({
            status: j.status,
            rows: j.rows_inserted,
            dups: j.duplicates_found,
            ms: 0,
            error: j.error,
          })
        }
      }).catch(() => {})
    }, 1000)
    return () => clearInterval(iv)
  }, [id])

  // Also capture duration from SSE 'done' event
  useEffect(() => {
    for (const ev of events) {
      if (ev.type === 'done' && !notifiedRef.current) {
        notifiedRef.current = true
        setDoneModal({ status: 'done', rows: ev.total_rows, dups: ev.duplicates, ms: ev.duration_ms })
      }
    }
  }, [events])

  // Derive progress from SSE events
  const progress = deriveProgress(events, job)

  return (
    <div className={styles.page}>
      {doneModal && (
        <div className={styles.overlay}>
          <div className={styles.modal}>
            {doneModal.status === 'done' ? (
              <>
                <div className={styles.modalIcon}>✅</div>
                <h3 className={styles.modalTitle}>{t('job.modal.done.title')}</h3>
                <div className={styles.modalStats}>
                  <div><span>{t('job.modal.done.rows')}</span><strong>{doneModal.rows.toLocaleString()}</strong></div>
                  <div><span>{t('job.modal.done.dups')}</span><strong>{doneModal.dups.toLocaleString()}</strong></div>
                  {doneModal.ms > 0 && (
                    <div>
                      <span>{t('job.modal.done.time')}</span>
                      <strong>{t('job.modal.done.seconds', { n: (doneModal.ms / 1000).toFixed(1) })}</strong>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <>
                <div className={styles.modalIcon}>❌</div>
                <h3 className={styles.modalTitleError}>{t('job.modal.error.title')}</h3>
                <p className={styles.modalError}>{doneModal.error}</p>
              </>
            )}
            <button className={styles.modalBtn} onClick={() => navigate('/')}>
              {t('job.modal.backBtn')}
            </button>
          </div>
        </div>
      )}
      <div className={styles.topBar}>
        <Link to="/">{t('job.backLink')}</Link>
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

          <div className={`card ${styles.progressCard}`}>
            <h2 className={styles.cardTitle}>{t('job.progress.title')}</h2>

            <ProgressSection
              label={t('job.progress.file', { current: progress.fileIndex, total: job.total_files })}
              sub={progress.currentFile || '—'}
              value={job.total_files > 0 ? progress.fileIndex / job.total_files : 0}
            />

            <ProgressSection
              label={t('job.progress.record', { read: progress.linesRead.toLocaleString(), total: progress.totalLines.toLocaleString() })}
              sub={t('job.progress.sub', {
                inserted: progress.rowsInserted.toLocaleString(),
                skipped: progress.rowsSkipped.toLocaleString(),
                dups: job.duplicates_found.toLocaleString(),
              })}
              value={progress.totalLines > 0 ? progress.linesRead / progress.totalLines : 0}
            />

            <div className={styles.eventLog}>
              {events.length === 0 && <p className={styles.emptyLog}>{t('job.eventLog.waiting')}</p>}
              {[...events].reverse().map((ev, i) => (
                <EventRow key={i} ev={ev} />
              ))}
            </div>
          </div>
        </>
      )}

      {!job && <p className={styles.loading}>{t('job.loading')}</p>}
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
  const { t } = useI18n()
  const [icon, color, text] = formatEvent(ev, t)
  return (
    <div className={styles.eventRow} style={{ borderLeftColor: color }}>
      <span className={styles.eventIcon}>{icon}</span>
      <span style={{ color }}>{text}</span>
    </div>
  )
}

function formatEvent(ev: SseEvent, t: (key: string, vars?: Record<string, string | number>) => string): [string, string, string] {
  switch (ev.type) {
    case 'file_start':
      return ['📂', '#90cdf4', t('job.event.file_start', {
        filename: ev.filename, index: ev.file_index, total: ev.total_files, lines: ev.total_lines.toLocaleString(),
      })]
    case 'progress':
      return ['📊', '#68d391', t('job.event.progress', {
        read: ev.lines_read.toLocaleString(), inserted: ev.rows_inserted.toLocaleString(), skipped: ev.rows_skipped.toLocaleString(),
      })]
    case 'schema_change':
      return ['➕', '#b794f4', t('job.event.schema_change', { column: ev.column, type: ev.sql_type })]
    case 'type_error_column':
      return ['⚠', '#f6ad55', t('job.event.type_error_column', { errorCol: ev.error_column, origCol: ev.original_column })]
    case 'duplicate':
      return ['🔁', '#fc8181', t('job.event.duplicate', { line: ev.line, action: ev.action })]
    case 'done':
      return ['✅', '#68d391', t('job.event.done', { rows: ev.total_rows.toLocaleString(), dups: ev.duplicates.toLocaleString(), ms: ev.duration_ms })]
    case 'error':
      return ['❌', '#fc8181', t('job.event.error', { message: ev.message })]
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
