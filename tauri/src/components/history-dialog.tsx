import { Cancel01Icon, Copy01Icon } from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import { useEffect, useId, useState } from "react"
import type { HistoryEntry } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface HistoryDialogProps {
  entries: HistoryEntry[]
  loading: boolean
  error: string | null
  onSearch: (query: string) => void
  onClose: () => void
}

export function HistoryDialog({ entries, loading, error, onSearch, onClose }: HistoryDialogProps) {
  const titleId = useId()
  const [query, setQuery] = useState("")
  const dialogRef = useDialogFocus(onClose, false)

  useEffect(() => {
    const timer = window.setTimeout(() => onSearch(query), 180)
    return () => window.clearTimeout(timer)
  }, [onSearch, query])

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section ref={dialogRef} className="dialog history-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="dialog-header">
          <div><span className="eyebrow">Local archive</span><h2 id={titleId}>Shared chat history</h2></div>
          <button className="dialog-close" type="button" aria-label="Close" onClick={onClose}><HugeiconsIcon icon={Cancel01Icon} size={20} /></button>
        </div>
        <p className="history-help">Search transcripts stored locally by each isolated account. Results never merge or transfer account ownership.</p>
        <label className="history-search"><span>Search chats</span><input autoFocus value={query} placeholder="Search every account" onChange={(event) => setQuery(event.currentTarget.value)} /></label>
        {error ? <p className="form-error" role="alert">{error}</p> : null}
        <div className="history-results" aria-busy={loading}>
          {entries.map((entry) => <article className="history-entry" key={entry.id}>
            <div><strong>{entry.profileName}</strong><small>{new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(entry.modifiedAt))}</small></div>
            <p>{entry.preview}</p>
            <button className="button secondary" type="button" onClick={() => void navigator.clipboard.writeText(entry.preview)}><HugeiconsIcon icon={Copy01Icon} size={16} />Copy preview</button>
          </article>)}
          {!loading && !error && entries.length === 0 ? <p className="history-empty">No locally saved chats match yet.</p> : null}
        </div>
      </section>
    </div>
  )
}
