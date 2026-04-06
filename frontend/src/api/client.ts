import type { Job, UploadResponse, TableInfo, DupMode } from '../types'

const BASE = '/api'

export function uploadFiles(
  files: File[],
  onProgress?: (pct: number) => void,
): Promise<UploadResponse[]> {
  return new Promise((resolve, reject) => {
    const fd = new FormData()
    for (const f of files) fd.append('file', f)

    const xhr = new XMLHttpRequest()
    xhr.open('POST', `${BASE}/upload`)

    if (onProgress) {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) onProgress(Math.round((e.loaded / e.total) * 100))
      }
    }

    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try { resolve(JSON.parse(xhr.responseText)) }
        catch { reject(new Error('Invalid response from server')) }
      } else {
        reject(new Error(xhr.responseText || `HTTP ${xhr.status}`))
      }
    }
    xhr.onerror = () => reject(new Error('ネットワークエラーが発生しました'))
    xhr.send(fd)
  })
}

export async function createJob(table_name: string, file_ids: string[], dup_mode: DupMode): Promise<Job> {
  const res = await fetch(`${BASE}/jobs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ table_name, file_ids, dup_mode }),
  })
  if (!res.ok) throw new Error(await res.text())
  return res.json()
}

export async function listJobs(): Promise<Job[]> {
  const res = await fetch(`${BASE}/jobs`)
  if (!res.ok) throw new Error(await res.text())
  return res.json()
}

export async function getJob(id: string): Promise<Job> {
  const res = await fetch(`${BASE}/jobs/${id}`)
  if (!res.ok) throw new Error(await res.text())
  return res.json()
}

export async function listTables(): Promise<TableInfo[]> {
  const res = await fetch(`${BASE}/tables`)
  if (!res.ok) throw new Error(await res.text())
  return res.json()
}

export async function truncateTable(name: string): Promise<void> {
  const res = await fetch(`${BASE}/tables/${encodeURIComponent(name)}/truncate`, { method: 'POST' })
  if (!res.ok) throw new Error(await res.text())
}

export async function dropTable(name: string): Promise<void> {
  const res = await fetch(`${BASE}/tables/${encodeURIComponent(name)}`, { method: 'DELETE' })
  if (!res.ok) throw new Error(await res.text())
}
