"use client"

import { useEffect, useState } from "react"

const palettes = ["Sage", "Ocean", "Sand", "Rose", "Midnight", "Coffee", "Graphite"] as const
const storageKey = "multi-codex-site-palette"
function savedPalette() {
  try { return localStorage.getItem(storageKey) ?? "sage" } catch { return "sage" }
}
export function PalettePicker() {
  const [palette, setPalette] = useState("sage")
  useEffect(() => {
    function sync() {
      const saved = savedPalette()
      const next = palettes.some(name => name.toLowerCase() === saved) ? saved : "sage"
      document.documentElement.dataset.palette = next
      setPalette(next)
    }
    sync()
    function changed(event: StorageEvent) { if (event.key === storageKey || event.key === null) sync() }
    window.addEventListener("storage", changed)
    return () => window.removeEventListener("storage", changed)
  }, [])
  return <select className="palette-picker" aria-label="Website palette" value={palette} onChange={event => {
    const next = event.target.value
    setPalette(next)
    document.documentElement.dataset.palette = next
    try { localStorage.setItem(storageKey, next) } catch { /* The selection still applies for this visit. */ }
  }}>{palettes.map(name => <option key={name} value={name.toLowerCase()}>{name}</option>)}</select>
}
