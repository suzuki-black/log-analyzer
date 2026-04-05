import { useEffect, useRef, useState } from 'react'
import type { SseEvent } from '../types'

export type SSEStatus = 'connecting' | 'open' | 'closed'

export function useSSE(jobId: string | null) {
  const [events, setEvents] = useState<SseEvent[]>([])
  const [status, setStatus] = useState<SSEStatus>('closed')
  const esRef = useRef<EventSource | null>(null)

  useEffect(() => {
    if (!jobId) return
    setEvents([])
    setStatus('connecting')

    const es = new EventSource(`/api/jobs/${jobId}/progress`)
    esRef.current = es

    es.onopen = () => setStatus('open')

    es.onmessage = (e) => {
      try {
        const ev: SseEvent = JSON.parse(e.data)
        setEvents((prev) => [...prev, ev])
        if (ev.type === 'done' || ev.type === 'error') {
          es.close()
          setStatus('closed')
        }
      } catch { /* ignore parse errors */ }
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

  return { events, status }
}
