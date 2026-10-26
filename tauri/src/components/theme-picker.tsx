import { colorThemes, type ColorTheme } from "../lib/themes"

export function ThemeChoices({ value, onChange }: { value: ColorTheme; onChange: (theme: ColorTheme) => void }) {
  return (
    <div className="theme-options">
      {colorThemes.map(theme => <button key={theme.id} type="button" className={`theme-option theme-preview-${theme.id}`} aria-pressed={value === theme.id} onClick={() => onChange(theme.id)}>
        <span className="theme-preview" aria-hidden="true"><i /><i /><i /></span>
        <span className="theme-option-name">{theme.name}<span aria-hidden="true">{value === theme.id ? "✓" : ""}</span></span>
      </button>)}
    </div>
  )
}
