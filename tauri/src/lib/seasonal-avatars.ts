import { useSyncExternalStore } from "react"
import dates from "./seasonal-dates.json"

// Background wardrobe triggers only, not an exposed calendar feature.
// Researched 2026-10-23: https://www.timeanddate.com/holidays/india/2026
// https://www.timeanddate.com/holidays/india/2027
// https://www.timeanddate.com/holidays/india/2028
// https://www.timeanddate.com/holidays/india/2029
// Diwali cross-check: https://www.drikpanchang.com/hindu-festivals/diwali/diwali.html
// Local civil dates follow India reference observances. Lunar / moon-sighting
// festivals can vary regionally. No remote fetches, no invented dates after 2029.
export const festivals = [
  { id: "halloween", name: "Halloween", looks: ["Midnight witch", "Velvet vampire", "Little mummy"] },
  { id: "christmas", name: "Christmas", looks: ["Santa’s helper", "Woodland reindeer", "Winter elf"] },
  { id: "diwali", name: "Diwali", looks: ["Embroidered silk", "Festival of lights", "Royal brocade"] },
  { id: "easter", name: "Easter", looks: ["Spring bunny", "Garden party", "Painted eggs"] },
  { id: "holi", name: "Holi", looks: ["Colour splash", "Pink gulal", "Rainbow scarf"] },
  { id: "eid", name: "Eid al-Fitr", looks: ["Emerald evening", "Moonlit gold", "Lantern night"] },
  { id: "eid-adha", name: "Eid al-Adha", looks: ["Emerald evening", "Moonlit gold", "Lantern night"] },
  { id: "lunar", name: "Lunar New Year", looks: ["Red brocade", "Golden lantern", "Jade celebration"] },
  { id: "onam", name: "Onam", looks: ["Ivory and gold", "Floral garland", "Harvest silk"] },
  { id: "pongal", name: "Pongal", looks: ["Harvest gold", "Woven silk", "Marigold morning"] },
  { id: "navratri", name: "Navratri", looks: ["Mirrorwork vest", "Festival scarf", "Embroidered silk"] },
  { id: "new-year", name: "New Year", looks: ["Midnight sparkle", "Golden party", "Confetti jacket"] },
  { id: "valentine", name: "Valentine’s Day", looks: ["Rose velvet", "Little sweetheart", "Blush scarf"] },
] as const
export type FestivalId = typeof festivals[number]["id"]
export function activeFestival(date: Date): FestivalId | null {
  const year = date.getFullYear()
  if (!(String(year) in dates)) return null
  const today = Date.UTC(year, date.getMonth(), date.getDate())
  const matches = [year - 1, year, year + 1].flatMap(eventYear => {
    const yearDates = dates[String(eventYear) as keyof typeof dates]
    if (!yearDates) return []
    return festivals.flatMap(f => {
      const [month, day] = yearDates[f.id].split("-").map(Number)
      const event = Date.UTC(eventYear, month - 1, day)
      // A short celebration: the preceding day, the event, and the following day.
      // Christmas gets Christmas Eve through Boxing Day; Navratri spans nine days.
      const before = f.id === "navratri" ? 0 : 1
      const after = f.id === "navratri" ? 8 : 1
      return today >= event - before * 86_400_000 && today <= event + after * 86_400_000 ? [{ id: f.id, distance: Math.abs(today - event) }] : []
    })
  })
  // Closest festival wins if windows overlap; declared order breaks a tie.
  return matches.sort((a, b) => a.distance - b.distance)[0]?.id ?? null
}
function localDay() { const d = new Date(); return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}` }
let day = localDay()
let timer: ReturnType<typeof setInterval> | undefined
const subscribers = new Set<() => void>()
function updateDay() { const next = localDay(); if (day !== next) { day = next; subscribers.forEach(fn => fn()) } }
function subscribe(callback: () => void) {
  subscribers.add(callback)
  if (subscribers.size === 1) { updateDay(); timer = setInterval(updateDay, 60_000); document.addEventListener("visibilitychange", updateDay) }
  return () => { subscribers.delete(callback); if (!subscribers.size) { clearInterval(timer); document.removeEventListener("visibilitychange", updateDay) } }
}
export function useSeasonalDay() { return useSyncExternalStore(subscribe, () => day) }
