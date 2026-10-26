import { expect, it } from "vitest"
import { activeFestival, festivals } from "./seasonal-avatars"
import dates from "./seasonal-dates.json"
it("uses researched moving festival dates for all three upcoming years", () => {
  for (const [year, month, day] of [[2027, 10, 29], [2028, 10, 17], [2029, 11, 5]]) expect(activeFestival(new Date(year, month - 1, day))).toBe("diwali")
  expect(activeFestival(new Date(2029, 3, 1))).toBe("easter")
  expect(activeFestival(new Date(2029, 2, 1))).toBe("holi")
})
it("gives each festival three costumes and a trigger for every supported year", () => {
  for (const f of festivals) {
    expect(f.looks).toHaveLength(3)
    for (const entries of Object.values(dates)) expect(entries[f.id]).toMatch(/^\d{2}-\d{2}$/)
  }
})
it("ends celebration windows and handles overlapping festivals", () => {
  expect(activeFestival(new Date(2027, 9, 29))).toBe("diwali")
  expect(activeFestival(new Date(2027, 9, 31))).toBe("halloween")
  expect(activeFestival(new Date(2028, 10, 3))).toBeNull()
  expect(activeFestival(new Date(2026, 11, 31))).toBe("new-year")
  expect(activeFestival(new Date(2029, 9, 23))).toBeNull()
  expect(activeFestival(new Date(2030, 10, 1))).toBeNull()
})
