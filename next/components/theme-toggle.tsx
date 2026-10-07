"use client"

import { useEffect, useRef, useState } from "react"
import { flushSync } from "react-dom"

type Theme = "light" | "dark"
const storageKey = "multi-codex-site-theme"

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>("light")
  const explicit = useRef(false)
  const nextReveal = useRef<"expand" | "contract">("expand")

  useEffect(() => {
    const system = window.matchMedia("(prefers-color-scheme: dark)")
    function apply(mode: Theme) {
      document.documentElement.dataset.theme = mode
      setTheme(mode)
    }
    function syncPreference() {
      let saved: string | null = null
      try { saved = localStorage.getItem(storageKey) } catch { /* Private browsing can block storage. */ }
      explicit.current = saved === "light" || saved === "dark"
      apply(saved === "light" || saved === "dark" ? saved : system.matches ? "dark" : "light")
    }
    function systemChanged(event: MediaQueryListEvent) {
      if (!explicit.current) apply(event.matches ? "dark" : "light")
    }
    function storageChanged(event: StorageEvent) {
      if (event.key === storageKey || event.key === null) syncPreference()
    }
    syncPreference()
    system.addEventListener("change", systemChanged)
    window.addEventListener("storage", storageChanged)
    return () => {
      system.removeEventListener("change", systemChanged)
      window.removeEventListener("storage", storageChanged)
    }
  }, [])

  function toggle(source: HTMLButtonElement) {
    const root = document.documentElement
    if (root.classList.contains("theme-transition")) return
    const next = root.dataset.theme === "dark" ? "light" : "dark"
    function apply() {
      explicit.current = true
      root.dataset.theme = next
      setTheme(next)
      try { localStorage.setItem(storageKey, next) } catch { /* The toggle still works for this visit. */ }
    }
    if (typeof document.startViewTransition !== "function" || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      apply()
      return
    }

    const rect = source.getBoundingClientRect()
    const x = rect.left + rect.width / 2
    const y = rect.top + rect.height / 2
    const radius = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y))
    root.style.setProperty("--theme-transition-x", `${x}px`)
    root.style.setProperty("--theme-transition-y", `${y}px`)
    root.style.setProperty("--theme-transition-radius", `${radius}px`)
    root.dataset.themeReveal = nextReveal.current
    root.classList.add("theme-transition")
    function cleanup() {
      root.classList.remove("theme-transition")
      delete root.dataset.themeReveal
      for (const property of ["x", "y", "radius"]) root.style.removeProperty(`--theme-transition-${property}`)
    }
    try {
      const transition = document.startViewTransition(() => flushSync(apply))
      transition.finished.then(cleanup, cleanup)
      transition.ready.then(() => {
        nextReveal.current = nextReveal.current === "expand" ? "contract" : "expand"
      }, () => { /* A skipped transition still applies the theme. */ })
    } catch {
      cleanup()
      apply()
    }
  }

  return <button type="button" className="theme-toggle" onClick={event => toggle(event.currentTarget)} aria-label="Toggle color theme" aria-pressed={theme === "dark"} title={`Use ${theme === "dark" ? "light" : "dark"} theme`}>
    <svg className="theme-moon" aria-hidden="true" viewBox="0 0 24 24"><path d="M20.5 13A8.5 8.5 0 0 1 11 3.5 8.5 8.5 0 1 0 20.5 13Z" /></svg>
    <svg className="theme-sun" aria-hidden="true" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" /></svg>
  </button>
}
