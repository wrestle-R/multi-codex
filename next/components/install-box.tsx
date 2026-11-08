import Link from "next/link"
import { installPage } from "@/lib/site"

export function InstallBox({ update = false }: { update?: boolean }) {
  return <div className="install-box">
    <p className="eyebrow">LINUX · MAC · WINDOWS</p>
    <Link className="button primary" href={installPage}>{update ? "Get the installer" : "Install Multi Codex"} <span aria-hidden="true">↓</span></Link>
    <p className="install-caption">One link. The right download and setup steps for your computer.</p>
  </div>
}
