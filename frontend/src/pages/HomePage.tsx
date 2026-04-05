import { useState, useRef, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { uploadFiles, createJob, listJobs, listTables } from '../api/client'
import type { UploadResponse, Job, TableInfo, DupMode } from '../types'
import styles from './HomePage.module.css'

function fmt(n: number) {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

// Detect .log and .gz pairs with the same base
function detectDuplicatePairs(files: File[]): string[] {
  const bases = new Map<string, string[]>()
  for (const f of files) {
    let base = f.name
    if (base.endsWith('.log')) base = base.slice(0, -4)
    else if (base.endsWith('.log.gz')) base = base.slice(0, -7)
    else if (base.endsWith('.gz')) base = base.slice(0, -3)
    if (!bases.has(base)) bases.set(base, [])
    bases.get(base)!.push(f.name)
  }
  const warnings: string[] = []
  for (const [base, names] of bases) {
    if (names.length > 1) warnings.push(`"${base}": ${names.join(', ')}`)
  }
  return warnings
}

export default function HomePage() {
  const navigate = useNavigate()
  const dropRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const [files, setFiles] = useState<File[]>([])
  const [uploads, setUploads] = useState<UploadResponse[]>([])
  const [tableName, setTableName] = useState('')
  const [dupMode, setDupMode] = useState<DupMode>('warn')
  const [existingTables, setExistingTables] = useState<TableInfo[]>([])
  const [recentJobs, setRecentJobs] = useState<Job[]>([])
  const [uploading, setUploading] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState('')
  const [pairWarnings, setPairWarnings] = useState<string[]>([])
  const [pairConfirmed, setPairConfirmed] = useState(false)
  const [dragging, setDragging] = useState(false)

  useEffect(() => {
    listTables().then(setExistingTables).catch(() => {})
    listJobs().then(setRecentJobs).catch(() => {})
  }, [])

  function addFiles(incoming: File[]) {
    const filtered = incoming.filter(f =>
      f.name.endsWith('.log') || f.name.endsWith('.gz')
    )
    const merged = [...files, ...filtered].filter(
      (f, i, arr) => arr.findIndex(g => g.name === f.name && g.size === f.size) === i
    )
    setFiles(merged)
    setUploads([])
    setPairConfirmed(false)
    const warnings = detectDuplicatePairs(merged)
    setPairWarnings(warnings)
  }

  function onDrop(e: React.DragEvent) {
    e.preventDefault()
    setDragging(false)
    addFiles(Array.from(e.dataTransfer.files))
  }

  async function handleUpload() {
    if (files.length === 0) return
    setUploading(true)
    setError('')
    try {
      const res = await uploadFiles(files)
      setUploads(res)
    } catch (e: any) {
      setError(e.message)
    } finally {
      setUploading(false)
    }
  }

  async function handleSubmit() {
    if (!tableName.trim() || uploads.length === 0) return
    setSubmitting(true)
    setError('')
    try {
      const job = await createJob(tableName.trim(), uploads.map(u => u.file_id), dupMode)
      navigate(`/jobs/${job.id}`)
    } catch (e: any) {
      setError(e.message)
      setSubmitting(false)
    }
  }

  const canUpload = files.length > 0 && (pairWarnings.length === 0 || pairConfirmed)
  const canSubmit = uploads.length > 0 && tableName.trim().length > 0

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1>Log Analyzer</h1>
        <p className={styles.sub}>NDJSON ログファイルを MySQL へ動的スキーマで取り込む</p>
      </header>

      <div className={styles.layout}>
        {/* Left: upload + config */}
        <div className={styles.main}>
          {/* Drop zone */}
          <div className="card">
            <h2 className={styles.sectionTitle}>ファイル選択</h2>
            <div
              ref={dropRef}
              className={`${styles.dropzone} ${dragging ? styles.dragover : ''}`}
              onDragOver={e => { e.preventDefault(); setDragging(true) }}
              onDragLeave={() => setDragging(false)}
              onDrop={onDrop}
              onClick={() => inputRef.current?.click()}
            >
              <span className={styles.dropIcon}>📂</span>
              <p>.log / .gz ファイルをドロップ、またはクリックして選択</p>
              <p className={styles.dropHint}>複数ファイル・ディレクトリ対応</p>
              <input
                ref={inputRef}
                type="file"
                multiple
                accept=".log,.gz"
                style={{ display: 'none' }}
                onChange={e => addFiles(Array.from(e.target.files ?? []))}
              />
            </div>

            {pairWarnings.length > 0 && !pairConfirmed && (
              <div className={styles.warning}>
                <strong>⚠ 同一ベース名のファイルが検出されました:</strong>
                <ul>{pairWarnings.map(w => <li key={w}>{w}</li>)}</ul>
                <p>同一ログの重複取り込みになる可能性があります。続行しますか？</p>
                <button className={styles.btnWarn} onClick={() => setPairConfirmed(true)}>続行する</button>
              </div>
            )}

            {files.length > 0 && (
              <table className={styles.fileTable}>
                <thead><tr><th>ファイル名</th><th>サイズ</th><th></th></tr></thead>
                <tbody>
                  {files.map((f, i) => (
                    <tr key={i}>
                      <td>{f.name}</td>
                      <td>{fmt(f.size)}</td>
                      <td>
                        <button className={styles.btnRemove} onClick={() => {
                          const next = files.filter((_, j) => j !== i)
                          setFiles(next)
                          setUploads([])
                          setPairWarnings(detectDuplicatePairs(next))
                          setPairConfirmed(false)
                        }}>✕</button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <button
              className={styles.btnPrimary}
              onClick={handleUpload}
              disabled={!canUpload || uploading || uploads.length > 0}
            >
              {uploading ? 'アップロード中...' : uploads.length > 0 ? '✓ アップロード済み' : 'サーバへアップロード'}
            </button>
          </div>

          {/* Config */}
          <div className="card" style={{ marginTop: 16 }}>
            <h2 className={styles.sectionTitle}>取り込み設定</h2>

            <div className={styles.field}>
              <label>テーブルベース名</label>
              <div className={styles.tableRow}>
                <input
                  value={tableName}
                  onChange={e => setTableName(e.target.value.replace(/[^a-zA-Z0-9_]/g, '_'))}
                  placeholder="例: access_log"
                />
                <span className={styles.suffix}>_la</span>
              </div>
              <p className={styles.hint}>実際のテーブル名: <code>{tableName || '(未入力)'}_la</code></p>
            </div>

            {existingTables.length > 0 && (
              <div className={styles.field}>
                <label>既存テーブルから選択</label>
                <select onChange={e => {
                  const v = e.target.value
                  if (v) setTableName(v.replace(/_la$/, ''))
                }}>
                  <option value="">-- 選択 --</option>
                  {existingTables.map(t => (
                    <option key={t.name} value={t.name}>{t.name}</option>
                  ))}
                </select>
              </div>
            )}

            <div className={styles.field}>
              <label>重複行の扱い</label>
              <div className={styles.radioGroup}>
                {([['warn', '警告して挿入'], ['flag_column', '_is_dup フラグを立てて挿入'], ['skip', 'スキップ']] as [DupMode, string][]).map(([v, label]) => (
                  <label key={v} className={styles.radio}>
                    <input type="radio" name="dup" value={v} checked={dupMode === v} onChange={() => setDupMode(v)} />
                    {label}
                  </label>
                ))}
              </div>
            </div>

            {error && <p className={styles.error}>{error}</p>}

            <button
              className={styles.btnPrimary}
              onClick={handleSubmit}
              disabled={!canSubmit || submitting}
            >
              {submitting ? '開始中...' : '取り込み開始'}
            </button>
          </div>
        </div>

        {/* Right: recent jobs */}
        <div className={styles.sidebar}>
          <div className="card">
            <h2 className={styles.sectionTitle}>最近のジョブ</h2>
            {recentJobs.length === 0
              ? <p className={styles.empty}>まだジョブはありません</p>
              : recentJobs.slice(0, 10).map(j => (
                  <a key={j.id} href={`/jobs/${j.id}`} className={styles.jobItem}>
                    <div className={styles.jobName}>{j.table_name}</div>
                    <StatusBadge status={j.status} />
                    <div className={styles.jobMeta}>{j.rows_inserted.toLocaleString()} 行</div>
                  </a>
                ))
            }
          </div>
        </div>
      </div>
    </div>
  )
}

function StatusBadge({ status }: { status: string }) {
  const map: Record<string, [string, string]> = {
    pending: ['#718096', 'PENDING'],
    running: ['#3182ce', 'RUNNING'],
    done: ['#38a169', 'DONE'],
    error: ['#e53e3e', 'ERROR'],
  }
  const [color, label] = map[status] ?? ['#718096', status]
  return <span className="tag" style={{ background: color + '33', color }}>{label}</span>
}
