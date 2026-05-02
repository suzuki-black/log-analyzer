import { createContext, useContext, useState, useEffect, type ReactNode, createElement } from 'react'
import en from './en.json'
import ja from './ja.json'

export type Lang = 'en' | 'ja'

const STORAGE_KEY = 'log-analyzer-lang'
const translations: Record<Lang, typeof en> = { en, ja }

function interpolate(str: string, vars?: Record<string, string | number>): string {
  if (!vars) return str
  return str.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? `{${k}}`))
}

function lookup(dict: any, key: string): string | undefined {
  const parts = key.split('.')
  let val: any = dict
  for (const p of parts) val = val?.[p]
  return typeof val === 'string' ? val : undefined
}

interface I18nContextValue {
  lang: Lang
  setLang: (lang: Lang) => void
  t: (key: string, vars?: Record<string, string | number>) => string
}

const I18nContext = createContext<I18nContextValue>({
  lang: 'en',
  setLang: () => {},
  t: (key) => key,
})

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(() => {
    const stored = localStorage.getItem(STORAGE_KEY)
    return stored === 'ja' ? 'ja' : 'en'
  })

  function setLang(next: Lang) {
    setLangState(next)
    localStorage.setItem(STORAGE_KEY, next)
  }

  function t(key: string, vars?: Record<string, string | number>): string {
    const val = lookup(translations[lang], key) ?? lookup(translations.en, key) ?? key
    return interpolate(val, vars)
  }

  return createElement(I18nContext.Provider, { value: { lang, setLang, t } }, children)
}

export function useI18n() {
  return useContext(I18nContext)
}

export function getLang(): Lang {
  const stored = localStorage.getItem(STORAGE_KEY)
  return stored === 'ja' ? 'ja' : 'en'
}
