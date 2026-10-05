import { BrandMark } from "./brand-mark"

export function WelcomeScreen({ platform, busy, error, onContinue }: { platform?: string; busy: boolean; error: string | null; onContinue: () => void }) {
  return <main className="welcome-screen">
    <div className="welcome-content">
      <div className="welcome-brand"><BrandMark /></div>
      <h1>Welcome to Multi Codex.</h1>
      <p>Separate accounts. One place to launch them.</p>
      <small className="welcome-platform">{platform === "macos" ? "Mac · Ready to go" : platform === "linux" ? "Linux · Ready to go" : "Getting ready"}</small>
      <button className="button primary" type="button" disabled={busy || (!platform && !error)} onClick={onContinue}>{busy ? "Getting ready…" : error && !platform ? "Try again" : "Get started"}<span aria-hidden="true">→</span></button>
      {error ? <p className="form-error" role="alert">{error}</p> : null}
      <p className="welcome-footer">Your default Codex login stays untouched.</p>
    </div>
  </main>
}
