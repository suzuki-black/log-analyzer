import { type ReactNode } from 'react'
import { NavLink } from 'react-router-dom'
import { useI18n } from '../i18n'
import styles from './Layout.module.css'

export default function Layout({ children }: { children: ReactNode }) {
  const { t } = useI18n()

  return (
    <>
      <nav className={styles.nav}>
        <div className={styles.navInner}>
          <NavLink to="/" className={styles.logo}>
            <span className={styles.logoDot} />
            Log Analyzer
          </NavLink>
          <div className={styles.navLinks}>
            <NavLink
              to="/"
              end
              className={({ isActive }) =>
                `${styles.navLink} ${isActive ? styles.navLinkActive : ''}`
              }
            >
              {t('nav.home')}
            </NavLink>
            <NavLink
              to="/settings"
              className={({ isActive }) =>
                `${styles.navLink} ${isActive ? styles.navLinkActive : ''}`
              }
            >
              {t('nav.settings')}
            </NavLink>
          </div>
        </div>
      </nav>
      <div className={styles.content}>{children}</div>
    </>
  )
}
