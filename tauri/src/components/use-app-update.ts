import { useCallback, useEffect, useRef, useState } from "react"
import { checkAppUpdate } from "../lib/desktop-api"

export function useAppUpdate(installing: boolean, refreshMinutes = 10) {
  const [version, setVersion] = useState<string | null>(null)
  const pending = useRef<Promise<void> | null>(null)
  const active = useRef(false)
  const generation = useRef(0)
  const installingRef = useRef(installing)
  installingRef.current = installing

  const check = useCallback((reset = false): Promise<void> => {
    if (installingRef.current) return Promise.resolve()
    if (reset) { generation.current++; pending.current = null; setVersion(null) }
    if (pending.current) return pending.current
    const epoch = generation.current
    const request = checkAppUpdate().then(next => {
      if (active.current && !installingRef.current && generation.current === epoch) setVersion(next)
    }).catch(() => {
      // A failed background check must not interrupt the app or erase an update
      // already discovered. Startup, refresh and reconnect will retry it.
    }).finally(() => {
      if (pending.current === request) pending.current = null
    })
    pending.current = request
    return request
  }, [])

  useEffect(() => {
    active.current = true
    const checkVisible = () => { if (document.visibilityState !== "hidden") void check() }
    void check()
    const timer = window.setInterval(() => void check(), refreshMinutes * 60_000)
    document.addEventListener("visibilitychange", checkVisible)
    window.addEventListener("online", checkVisible)
    return () => {
      active.current = false
      window.clearInterval(timer)
      document.removeEventListener("visibilitychange", checkVisible)
      window.removeEventListener("online", checkVisible)
    }
  }, [check, refreshMinutes])

  return { version, check, clear: () => setVersion(null) }
}
