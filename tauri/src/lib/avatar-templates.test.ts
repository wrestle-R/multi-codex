import { expect, it } from "vitest"
import { avatarTemplates, balanceTemplates, templateFor } from "./avatar-templates"
it("balances all six templates evenly while keeping existing assignments stable", () => {
  const ids = Array.from({ length: 41 }, (_, index) => `account-${index}`)
  const assigned = balanceTemplates(ids, {})
  const counts = avatarTemplates.map((_, index) => Object.values(assigned).filter(n => n === index).length)
  expect(Math.max(...counts) - Math.min(...counts)).toBeLessThanOrEqual(1)
  const added = balanceTemplates([...ids, "new-account"], assigned)
  for (const id of ids) expect(added[id]).toBe(assigned[id])
  expect(new Set(Object.values(assigned)).size).toBe(6)
})
it("always resolves the same colour to the same template regardless of account seed or previous shape", () => {
  for (const [index, template] of avatarTemplates.entries()) {
    expect(templateFor({ ...template, seed: 999, kind: "blob" })).toBe(index)
    expect(templateFor({ ...template, seed: 7 })).toBe(index)
  }
  expect(balanceTemplates(["a", "b", "c"], {})).toEqual(balanceTemplates(["c", "a", "b"], {}))
})
