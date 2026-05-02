import { useI18n, type Lang } from '../i18n'
import styles from './SettingsPage.module.css'

export default function SettingsPage() {
  const { t, lang, setLang } = useI18n()

  const langs: { value: Lang; label: string }[] = [
    { value: 'en', label: t('settings.language.en') },
    { value: 'ja', label: t('settings.language.ja') },
  ]

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h1>{t('settings.title')}</h1>
      </div>

      <div className="card">
        <div className={styles.sectionTitle}>{t('settings.language.label')}</div>
        <div className={styles.row}>
          <span className={styles.rowLabel}>{t('settings.language.label')}</span>
          <div className={styles.radioGroup}>
            {langs.map(({ value, label }) => (
              <button
                key={value}
                className={`${styles.radioBtn} ${lang === value ? styles.radioBtnActive : ''}`}
                onClick={() => setLang(value)}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
