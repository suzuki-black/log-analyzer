import { useEffect, useRef, useState } from 'react'
import type { SseEvent } from '../types'

export type SSEStatus = 'connecting' | 'open' | 'closed'

/** イベントログに保持する最大件数 */
const MAX_LOG_EVENTS = 200

/**
 * ログに表示するイベント種別。
 * progress / duplicate / duplicate_batch は件数が膨大になるため
 * ログには残さず専用 state で管理する。
 */
const LOG_TYPES = new Set<SseEvent['type']>([
  'file_start', 'schema_change', 'type_error_column', 'done', 'error',
])

export type ProgressSnapshot = {
  lines_read: number
  rows_inserted: number
  rows_skipped: number
  total_lines: number
}

export function useSSE(jobId: string | null) {
  /** ログ表示用イベント（最大 MAX_LOG_EVENTS 件） */
  const [logEvents, setLogEvents] = useState<SseEvent[]>([])
  /** 最新の Progress スナップショット（常に最新1件のみ保持） */
  const [latestProgress, setLatestProgress] = useState<ProgressSnapshot | null>(null)
  /** 受信した全イベント数（ログに残らない分も含む） */
  const [totalReceived, setTotalReceived] = useState(0)
  const [status, setStatus] = useState<SSEStatus>('closed')
  const esRef = useRef<EventSource | null>(null)

  useEffect(() => {
    if (!jobId) return
    setLogEvents([])
    setLatestProgress(null)
    setTotalReceived(0)
    setStatus('connecting')

    const es = new EventSource(`/api/jobs/${jobId}/progress`)
    esRef.current = es

    es.onopen = () => setStatus('open')

    es.onmessage = (e) => {
      try {
        const ev: SseEvent = JSON.parse(e.data)
        setTotalReceived((n) => n + 1)

        if (ev.type === 'progress') {
          // Progress は専用 state に上書き保存（ログに積まない）
          setLatestProgress({
            lines_read: ev.lines_read,
            rows_inserted: ev.rows_inserted,
            rows_skipped: ev.rows_skipped,
            total_lines: ev.total_lines,
          })
          return
        }

        if (ev.type === 'duplicate' || ev.type === 'duplicate_batch') {
          // dup 件数は job.duplicates_found で管理、ログには積まない
          return
        }

        if (LOG_TYPES.has(ev.type)) {
          setLogEvents((prev) => {
            const next = [...prev, ev]
            return next.length > MAX_LOG_EVENTS
              ? next.slice(next.length - MAX_LOG_EVENTS)
              : next
          })
        }

        if (ev.type === 'done' || ev.type === 'error') {
          es.close()
          setStatus('closed')
        }
      } catch {
        /* ignore parse errors */
      }
    }

    es.onerror = () => {
      es.close()
      setStatus('closed')
    }

    return () => {
      es.close()
      setStatus('closed')
    }
  }, [jobId])

  return { logEvents, latestProgress, totalReceived, status }
}
