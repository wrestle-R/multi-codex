export async function waitForReady<T extends { ready: boolean; connectionFailed?: boolean; reason?: string | null }>(
  read: () => Promise<T>, timeoutMs = 15000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const state = await read();
    if (state.connectionFailed) throw new Error(state.reason || 'Codex backend disconnected. Save your work and reopen Codex, then retry.');
    if (state.ready) return state;
    if (Date.now() >= deadline) throw new Error('Codex did not finish connecting. Open its panel, wait for startup to finish, then retry.');
    await new Promise(resolve => setTimeout(resolve, Math.min(100, Math.max(1, deadline - Date.now()))));
  }
}
