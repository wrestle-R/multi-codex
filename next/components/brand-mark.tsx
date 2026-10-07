// Keep the same overlapping terminal mark used inside the desktop app.
export function BrandMark({ small = false }: { small?: boolean }) {
  return <span className={`brand-icon${small ? " small" : ""}`} aria-hidden="true"><span /><span /><i>&gt;_</i></span>
}
