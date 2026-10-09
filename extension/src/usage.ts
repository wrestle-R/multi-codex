export function usageSummary(result: any, now = Date.now()) {
  const windows = [result?.rateLimits?.primary, result?.rateLimits?.secondary].filter(Boolean);
  const summaries = windows.map((window: any) => {
    const duration = window.windowDurationMins;
    const label = duration >= 10080 ? 'Weekly' : duration ? `${duration / 60}h` : 'Usage';
    if (!Number.isFinite(window.usedPercent)) return `${label}: unavailable`;
    const left = Math.max(0, Math.min(100, 100 - window.usedPercent));
    const reset = Number(window.resetsAt) * 1000;
    const minutes = Math.ceil((reset - now) / 60000);
    const time = Number.isFinite(reset) && reset > now ? minutes >= 1440 ? `${Math.ceil(minutes / 1440)}d` : minutes >= 60 ? `${Math.ceil(minutes / 60)}h` : `${minutes}m` : '';
    return `${label}: ${Math.round(left)}% left${time ? ` · resets in ${time}` : ''}`;
  });
  return summaries.join(' · ') || 'Usage unavailable';
}

/** Bound background backends and keep one failed account from cancelling the rest. */
export async function refreshAllAccounts<T>(accounts: T[], check: (account: T) => Promise<unknown>, complete: (account: T, error?: unknown) => void, concurrency = 2) {
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(concurrency, accounts.length) }, async () => {
    while (next < accounts.length) {
      const account = accounts[next++];
      try { await check(account); complete(account); } catch (error) { complete(account, error); }
    }
  }));
}
