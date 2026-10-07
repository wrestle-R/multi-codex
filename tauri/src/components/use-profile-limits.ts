import { useCallback, useEffect, useRef, useState } from "react"
import { checkProfileLimits } from "../lib/desktop-api"
import { readLimitsCache, saveLimitsCache } from "../lib/limits-cache"
import type { Profile } from "../lib/types"

export function useProfileLimits(profiles: Profile[], ready: boolean) {
  const [limitChecks, setLimitChecks] = useState(readLimitsCache)
  const latestProfiles = useRef(profiles)
  useEffect(() => { latestProfiles.current = profiles }, [profiles])
  const started = useRef(new Set<string>())
  const requests = useRef(new Map<string, Promise<void>>())
  const handleCheckLimits = useCallback((profile: Profile): Promise<void> => {
    if (profile.authMode.toLowerCase() !== "chatgpt") return Promise.resolve()
    const pending = requests.current.get(profile.id)
    if (pending) return pending
    setLimitChecks(current => ({ ...current, [profile.id]: { ...current[profile.id], loading: true, error: undefined } }))
    const request = (async () => {
      try {
        const data = await Promise.resolve().then(() => checkProfileLimits(profile.id))
        setLimitChecks(current => ({ ...current, [profile.id]: { loading: false, data } }))
      } catch (error) {
        setLimitChecks(current => ({ ...current, [profile.id]: {
          ...current[profile.id], loading: false, error: error instanceof Error ? error.message : String(error),
        } }))
      } finally { requests.current.delete(profile.id) }
    })()
    requests.current.set(profile.id, request)
    return request
  }, [])
  useEffect(() => {
    if (!ready) return
    for (const profile of profiles) {
      if (profile.authMode.toLowerCase() !== "chatgpt" || started.current.has(profile.id)) continue
      started.current.add(profile.id)
      void handleCheckLimits(profile)
    }
  }, [profiles, ready, handleCheckLimits])
  useEffect(() => {
    if (ready) saveLimitsCache(limitChecks, new Set(profiles.map(profile => profile.id)))
  }, [limitChecks, profiles, ready])
  useEffect(() => {
    if (!ready) return
    const refreshVisible = () => {
      if (document.visibilityState !== "hidden") void Promise.all(latestProfiles.current.map(handleCheckLimits))
    }
    // Refresh expiry details while open, and after returning from the background.
    const timer = window.setInterval(refreshVisible, 15 * 60_000)
    document.addEventListener("visibilitychange", refreshVisible)
    return () => {
      window.clearInterval(timer)
      document.removeEventListener("visibilitychange", refreshVisible)
    }
  }, [ready, handleCheckLimits])
  const refreshingAllLimits = profiles.some(profile => limitChecks[profile.id]?.loading)
  const handleRefreshAllLimits = () => Promise.all(profiles.map(handleCheckLimits))
  return { limitChecks, refreshingAllLimits, handleCheckLimits, handleRefreshAllLimits }
}
