export const colorThemes = [
  { id: "sage", name: "Sage" },
  { id: "ocean", name: "Ocean" },
  { id: "sand", name: "Sand" },
  { id: "rose", name: "Rose" },
  { id: "plum", name: "Plum" },
  { id: "orange", name: "Orange" },
] as const
export type ColorTheme = typeof colorThemes[number]["id"]
export function initialColorTheme(): ColorTheme {
  const saved = localStorage.getItem("multi-codex-color-theme")
  if (saved === "slate") return "orange"
  return colorThemes.find(theme => theme.id === saved)?.id ?? "orange"
}
