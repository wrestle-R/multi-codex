import type { Message } from './protocol';

// One selected local conversation per client window. Backend prefetches and
// metadata reads cannot turn retained history into additional visible chats.
export class OrbitSelection {
  selected: string | null = null;
  readonly testEvents: string[] = [];
  private trace(event: string) {
    if (process.env.MULTI_CODEX_TEST_MODE !== '1') return;
    this.testEvents.push(event); if (this.testEvents.length > 24) this.testEvents.shift();
  }
  private generation = 0;
  private ephemeral = new Set<string>();
  private pending = new Map<string | number, { method: string; threadId?: string; generation: number; ephemeral?: boolean }>();
  outgoing(message: Message) {
    if (['thread/start', 'thread/startAeon', 'thread/fork'].includes(message.method ?? '')) {
      // The frontend prepares a fresh composer, often with extra cached starts.
      // Invalidate the old view without treating any prewarmed result as open.
      const ephemeral = message.params?.ephemeral === true;
      if (!ephemeral) { this.selected = null; this.generation++; }
      this.trace(`send ${message.method} ephemeral=${ephemeral}`);
      if (message.id !== undefined) this.pending.set(message.id, { method: message.method!, generation: this.generation, ephemeral });
      return;
    }
    // The editor prewarms extra thread/start results. A submitted turn or an
    // explicit resume selects the conversation; creating a cache does not.
    if (message.id === undefined || !message.method || !['turn/start', 'thread/resume', 'thread/unsubscribe', 'thread/archive', 'thread/delete'].includes(message.method)) return;
    this.trace(`send ${message.method}`);
    this.pending.set(message.id, { method: message.method, threadId: message.params?.threadId, generation: this.generation });
    if (this.pending.size > 4096) this.pending.delete(this.pending.keys().next().value!);
  }
  incoming(message: Message) {
    if (message.method?.startsWith('thread/')) this.trace(`receive ${message.method} ${message.params?.status?.type ?? ''}`);
    // Engine eviction does not close the selected editor conversation. Only
    // explicit UI lifecycle requests/events clear its visible selection.
    if (['thread/closed', 'thread/deleted', 'thread/archived'].includes(message.method ?? '')) {
      if (message.params?.threadId === this.selected) this.selected = null;
    }
    if (message.method || message.id === undefined) return;
    const request = this.pending.get(message.id); this.pending.delete(message.id);
    if (request) this.trace(`reply ${request.method} ${message.error ? 'error' : 'ok'}`);
    if (!request || message.error || request.generation !== this.generation) return;
    if (['thread/start', 'thread/startAeon', 'thread/fork'].includes(request.method)) {
      const id = message.result?.thread?.id;
      if (request.ephemeral && typeof id === 'string') {
        this.ephemeral.add(id);
        if (this.ephemeral.size > 4096) this.ephemeral.delete(this.ephemeral.values().next().value!);
      }
      return;
    }
    if (['thread/unsubscribe', 'thread/archive', 'thread/delete'].includes(request.method)) {
      if (request.threadId === this.selected) this.selected = null;
    } else {
      const id = request.method === 'turn/start' ? request.threadId : message.result?.thread?.id;
      if (typeof id === 'string' && !this.ephemeral.has(id) && /^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(id)) this.selected = id;
    }
  }
  reset() { this.selected = null; this.pending.clear(); this.ephemeral.clear(); this.generation++; }
}
