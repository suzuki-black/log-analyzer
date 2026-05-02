import { useState, useRef, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { uploadFiles, createJob, listJobs, listTables, truncateTable, dropTable } from '../api/client'
import type { UploadResponse, Job, TableInfo, DupMode } from '../types'
import { useI18n } from '../i18n'
import styles from './HomePage.module.css'

function fmt(n: number) {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

// Detect .log and .gz pairs with the same base (uses full path when available)
function detectDuplicatePairs(files: File[]): string[] {
  const bases = new Map<string, string[]>()
  for (const f of files) {
    let base = filePath(f)
    if (base.endsWith('.log')) base = base.slice(0, -4)
    else if (base.endsWith('.log.gz')) base = base.slice(0, -7)
    else if (base.endsWith('.gz')) base = base.slice(0, -3)
    if (!bases.has(base)) bases.set(base, [])
    bases.get(base)!.push(filePath(f))
  }
  const warnings: string[] = []
  for (const [base, names] of bases) {
    if (names.length > 1) warnings.push(`"${base}": ${names.join(', ')}`)
  }
  return warnings
}

// Recursively collect all File objects from a dropped DataTransfer (handles directories)
async function collectDroppedFiles(dt: DataTransfer): Promise<File[]> {
  const entries: FileSystemEntry[] = []
  for (let i = 0; i < dt.items.length; i++) {
    const entry = dt.items[i].webkitGetAsEntry?.()
    if (entry) entries.push(entry)
  }

  async function readEntry(entry: FileSystemEntry): Promise<File[]> {
    if (entry.isFile) {
      return new Promise(resolve =>
        (entry as FileSystemFileEntry).file(f => resolve([f]), () => resolve([]))
      )
    }
    if (entry.isDirectory) {
      const reader = (entry as FileSystemDirectoryEntry).createReader()
      const all: FileSystemEntry[] = []
      // readEntries returns ≤100 at a time; loop until empty
      while (true) {
        const batch: FileSystemEntry[] = await new Promise(res => reader.readEntries(res, () => res([])))
        if (batch.length === 0) break
        all.push(...batch)
      }
      return (await Promise.all(all.map(readEntry))).flat()
    }
    return []
  }

  return (await Promise.all(entries.map(readEntry))).flat()
}

// Display name: prefer webkitRelativePath (set when using directory input or drag-dir)
function filePath(f: File): string {
  return (f as any).webkitRelativePath || f.name
}

export default function HomePage() {
  const navigate = useNavigate()
  const { t } = useI18n()
  const dropRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const dirInputRef = useRef<HTMLInputElement>(null)

  const [files, setFiles] = useState<File[]>([])
  const [uploads, setUploads] = useState<UploadResponse[]>([])
  const [tableName, setTableName] = useState('')
  const [dupMode, setDupMode] = useState<DupMode>('warn')
  const [existingTables, setExistingTables] = useState<TableInfo[]>([])
  const [recentJobs, setRecentJobs] = useState<Job[]>([])
  const [uploading, setUploading] = useState(false)
  const [uploadPct, setUploadPct] = useState(0)
  const [uploadDone, setUploadDone] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState('')
  const [pairWarnings, setPairWarnings] = useState<string[]>([])
  const [pairConfirmed, setPairConfirmed] = useState(false)
  const [dragging, setDragging] = useState(false)

  useEffect(() => {
    listTables().then(setExistingTables).catch(() => {})
    listJobs().then(setRecentJobs).catch(() => {})
  }, [])

  // webkitdirectory is not recognized by React JSX — set it imperatively
  useEffect(() => {
    dirInputRef.current?.setAttribute('webkitdirectory', '')
  }, [])

  function addFiles(incoming: File[]) {
    const filtered = incoming.filter(f =>
      f.name.endsWith('.log') || f.name.endsWith('.gz')
    )
    const merged = [...files, ...filtered].filter(
      (f, i, arr) => arr.findIndex(g => filePath(g) === filePath(f) && g.size === f.size) === i
    )
    setFiles(merged)
    setUploads([])
    setPairConfirmed(false)
    setPairWarnings(detectDuplicatePairs(merged))
  }

  async function onDrop(e: React.DragEvent) {
    e.preventDefault()
    setDragging(false)
    const collected = await collectDroppedFiles(e.dataTransfer)
    addFiles(collected.length > 0 ? collected : Array.from(e.dataTransfer.files))
  }

  async function handleUpload() {
    if (files.length === 0) return
    setUploading(true)
    setUploadPct(0)
    setError('')
    try {
      const res = await uploadFiles(files, (pct) => setUploadPct(pct))
      setUploads(res)
      setUploadDone(true)
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
      {uploadDone && (
        <div className={styles.dialogOverlay} onClick={() => setUploadDone(false)}>
          <div className={styles.dialog} onClick={e => e.stopPropagation()}>
            <div className={styles.dialogIcon}>✅</div>
            <h3>{t('home.dialog.uploadDone.title')}</h3>
            <p>
              {t('home.dialog.uploadDone.body', { n: uploads.length })}<br />
              {t('home.dialog.uploadDone.body2')}
            </p>
            <button className={styles.btnPrimary} onClick={() => setUploadDone(false)}>
              {t('home.dialog.uploadDone.ok')}
            </button>
          </div>
        </div>
      )}

      <header className={styles.header}>
        <h1>Log Analyzer</h1>
        <p className={styles.sub}>{t('app.subtitle')}</p>
      </header>

      <div className={styles.layout}>
        {/* Left: upload + config */}
        <div className={styles.main}>
          {/* Drop zone */}
          <div className="card">
            <h2 className={styles.sectionTitle}>{t('home.fileSelect.title')}</h2>
            <div
              ref={dropRef}
              className={`${styles.dropzone} ${dragging ? styles.dragover : ''}`}
              onDragOver={e => { e.preventDefault(); setDragging(true) }}
              onDragLeave={() => setDragging(false)}
              onDrop={onDrop}
            >
              <span className={styles.dropIcon}>📂</span>
              <p>{t('home.fileSelect.dropHint')}</p>
              <div className={styles.selectBtns}>
                <button className={styles.btnSelect} onClick={() => inputRef.current?.click()}>
                  {t('home.fileSelect.btnFile')}
                </button>
                <button className={styles.btnSelect} onClick={() => dirInputRef.current?.click()}>
                  {t('home.fileSelect.btnFolder')}
                </button>
              </div>
              <input
                ref={inputRef}
                type="file"
                multiple
                accept=".log,.gz"
                style={{ display: 'none' }}
                onChange={e => { addFiles(Array.from(e.target.files ?? [])); e.target.value = '' }}
              />
              <input
                ref={dirInputRef}
                type="file"
                multiple
                style={{ display: 'none' }}
                onChange={e => { addFiles(Array.from(e.target.files ?? [])); e.target.value = '' }}
              />
            </div>

            {pairWarnings.length > 0 && !pairConfirmed && (
              <div className={styles.warning}>
                <strong>{t('home.fileSelect.pairWarning.title')}</strong>
                <ul>{pairWarnings.map(w => <li key={w}>{w}</li>)}</ul>
                <p>{t('home.fileSelect.pairWarning.body')}</p>
                <button className={styles.btnWarn} onClick={() => setPairConfirmed(true)}>
                  {t('home.fileSelect.pairWarning.confirm')}
                </button>
              </div>
            )}

            {files.length > 0 && (
              <table className={styles.fileTable}>
                <thead>
                  <tr>
                    <th>{t('home.fileSelect.tableHeader.name')}</th>
                    <th>{t('home.fileSelect.tableHeader.size')}</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {files.map((f, i) => (
                    <tr key={i}>
                      <td className={styles.pathCell}>{filePath(f)}</td>
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
              {uploading
                ? t('home.fileSelect.btnUploading', { pct: uploadPct })
                : uploads.length > 0
                  ? t('home.fileSelect.btnUploaded')
                  : t('home.fileSelect.btnUpload')}
            </button>
            {uploading && (
              <div className={styles.uploadProgressTrack}>
                <div className={styles.uploadProgressFill} style={{ width: `${uploadPct}%` }} />
              </div>
            )}
          </div>

          {/* Config */}
          <div className="card" style={{ marginTop: 16 }}>
            <h2 className={styles.sectionTitle}>{t('home.config.title')}</h2>

            <div className={styles.field}>
              <label>{t('home.config.tableName.label')}</label>
              <div className={styles.tableRow}>
                <input
                  value={tableName}
                  onChange={e => setTableName(e.target.value.replace(/[^a-zA-Z0-9_]/g, '_'))}
                  placeholder={t('home.config.tableName.placeholder')}
                />
                <span className={styles.suffix}>_la</span>
              </div>
              <p className={styles.hint}>
                {t('home.config.tableName.hint')}{' '}
                <code>{tableName || t('home.config.tableName.hintEmpty')}_la</code>
              </p>
            </div>

            {existingTables.length > 0 && (
              <div className={styles.field}>
                <label>{t('home.config.existingTable.label')}</label>
                <select onChange={e => {
                  const v = e.target.value
                  if (v) setTableName(v.replace(/_la$/, ''))
                }}>
                  <option value="">{t('home.config.existingTable.placeholder')}</option>
                  {existingTables.map(tbl => (
                    <option key={tbl.name} value={tbl.name}>{tbl.name}</option>
                  ))}
                </select>
              </div>
            )}

            <div className={styles.field}>
              <label>{t('home.config.dupMode.label')}</label>
              <div className={styles.radioGroup}>
                {(['warn', 'flag_column', 'skip'] as DupMode[]).map(v => (
                  <label key={v} className={styles.radio}>
                    <input type="radio" name="dup" value={v} checked={dupMode === v} onChange={() => setDupMode(v)} />
                    {t(`home.config.dupMode.${v}`)}
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
              {submitting ? t('home.config.btnStarting') : t('home.config.btnStart')}
            </button>
          </div>
        </div>

        {/* Right: recent jobs + table management */}
        <div className={styles.sidebar}>
          <div className="card">
            <h2 className={styles.sectionTitle}>{t('home.recentJobs.title')}</h2>
            {recentJobs.length === 0
              ? <p className={styles.empty}>{t('home.recentJobs.empty')}</p>
              : recentJobs.slice(0, 10).map(j => (
                  <a key={j.id} href={`/jobs/${j.id}`} className={styles.jobItem}>
                    <div className={styles.jobName}>{j.table_name}</div>
                    <StatusBadge status={j.status} />
                    <div className={styles.jobMeta}>{t('home.recentJobs.rows', { n: j.rows_inserted.toLocaleString() })}</div>
                  </a>
                ))
            }
          </div>

          <div className="card" style={{ marginTop: 16 }}>
            <h2 className={styles.sectionTitle}>{t('home.tableManage.title')}</h2>
            <TableManager
              tables={existingTables}
              onRefresh={() => listTables().then(setExistingTables).catch(() => {})}
            />
          </div>
        </div>
      </div>
    </div>
  )
}

function TableManager({ tables, onRefresh }: { tables: TableInfo[], onRefresh: () => void }) {
  const { t } = useI18n()
  const [confirmState, setConfirmState] = useState<{ name: string; action: 'truncate' | 'drop' } | null>(null)
  const [opError, setOpError] = useState('')
  const [working, setWorking] = useState(false)

  async function execute() {
    if (!confirmState) return
    setWorking(true)
    setOpError('')
    try {
      if (confirmState.action === 'truncate') {
        await truncateTable(confirmState.name)
      } else {
        await dropTable(confirmState.name)
      }
      setConfirmState(null)
      onRefresh()
    } catch (e: any) {
      setOpError(e.message)
    } finally {
      setWorking(false)
    }
  }

  if (tables.length === 0) return <p className={styles.empty}>{t('home.tableManage.empty')}</p>

  return (
    <>
      {confirmState && (
        <div className={styles.dialogOverlay} onClick={() => !working && setConfirmState(null)}>
          <div className={styles.dialog} onClick={e => e.stopPropagation()}>
            <div className={styles.dialogIcon}>{confirmState.action === 'truncate' ? '🔄' : '🗑️'}</div>
            <h3 style={{ color: confirmState.action === 'truncate' ? '#f6ad55' : '#fc8181' }}>
              {confirmState.action === 'truncate'
                ? t('home.dialog.confirm.truncateTitle')
                : t('home.dialog.confirm.dropTitle')}
            </h3>
            <p>
              <code style={{ color: '#90cdf4', background: '#2d3748', padding: '2px 6px', borderRadius: 4 }}>
                {confirmState.name}
              </code>
              <br /><br />
              {confirmState.action === 'truncate'
                ? t('home.dialog.confirm.truncateBody')
                : t('home.dialog.confirm.dropBody')}
            </p>
            {opError && <p className={styles.error}>{opError}</p>}
            <div style={{ display: 'flex', gap: 10 }}>
              <button
                className={styles.btnPrimary}
                style={{ background: confirmState.action === 'truncate' ? '#c05621' : '#c53030' }}
                onClick={execute}
                disabled={working}
              >
                {working ? t('home.dialog.confirm.working') : t('home.dialog.confirm.execute')}
              </button>
              <button
                className={styles.btnPrimary}
                style={{ background: '#4a5568' }}
                onClick={() => setConfirmState(null)}
                disabled={working}
              >
                {t('home.dialog.confirm.cancel')}
              </button>
            </div>
          </div>
        </div>
      )}
      <div className={styles.tableList}>
        {tables.map(tbl => (
          <div key={tbl.name} className={styles.tableRow2}>
            <span className={styles.tableName2}>{tbl.name}</span>
            <div className={styles.tableActions}>
              <button
                className={styles.btnTableAction}
                title={t('home.tableManage.btnResetTitle')}
                onClick={() => { setOpError(''); setConfirmState({ name: tbl.name, action: 'truncate' }) }}
              >{t('home.tableManage.btnReset')}</button>
              <button
                className={`${styles.btnTableAction} ${styles.btnTableDrop}`}
                title={t('home.tableManage.btnDropTitle')}
                onClick={() => { setOpError(''); setConfirmState({ name: tbl.name, action: 'drop' }) }}
              >{t('home.tableManage.btnDrop')}</button>
            </div>
          </div>
        ))}
      </div>
    </>
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
