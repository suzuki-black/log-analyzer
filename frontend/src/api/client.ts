import type { Job, UploadResponse, TableInfo, DupMode } from '../types'

const BASE = '/api'

export async function uploadFiles(files: File[]): Promise<UploadResponse[]> {
  const fd = new FormData()
  for (const f of files) fd.append('file', f)
  const res = await fetch(`${BASE}/upload`, { method: 'POST', body: fd })
  if (!res.ok) throw new Error(await res.text())
  return res.json()
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
