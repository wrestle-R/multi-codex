import type { Message } from './protocol';

const WORK_METHODS = new Set(['turn/start', 'turn/steer', 'turn/addUserMessage', 'review/start', 'thread/start', 'thread/startAeon', 'thread/resume', 'thread/fork', 'thread/shellCommand', 'thread/compact/start', 'thread/queue/add', 'thread/queue/enqueue', 'thread/realtime/start', 'thread/realtime/appendAudio', 'thread/realtime/appendText', 'thread/realtime/appendSpeech', 'command/exec', 'process/start']);
export class ActivityGuard {
  ready = false;
  switching = false;
  unknown = false;
  readonly activeTurns = new Set<string>();
  readonly activeThreads = new Set<string>();
  readonly pendingWork = new Set<string | number>();
  readonly pendingApprovals = new Set<string | number>();
  readonly queuedThreads = new Set<string>();
  readonly processes = new Set<string>();
  private workMethods = new Map<string | number, string>();
  readonly unverifiedQueues = new Set<string>();
  verifyQueue(threadId: string, queue: unknown[]) {
    this.unverifiedQueues.delete(threadId);
    if (queue.length) this.queuedThreads.add(threadId); else this.queuedThreads.delete(threadId);
  }
  isWork(method?: string) { return !!method && (WORK_METHODS.has(method) || /^thread\/queue\/(?:add|enqueue|submit|start)$/.test(method)); }
  outgoing(message: Message) {
    if (this.isWork(message.method) && message.id !== undefined) { this.pendingWork.add(message.id); this.workMethods.set(message.id, message.method!); }
    if (!message.method && message.id !== undefined) this.pendingApprovals.delete(message.id);
  }
  incoming(message: Message) {
    const params = message.params ?? {};
    if (!message.method && message.id !== undefined && this.pendingWork.has(message.id)) {
      const method = this.workMethods.get(message.id); this.pendingWork.delete(message.id); this.workMethods.delete(message.id);
      if (!message.error) {
        const turn = message.result?.turn;
        if (turn?.status === 'inProgress') this.activeTurns.add(turn.id);
        else if (method === 'thread/compact/start') this.activeThreads.add(message.result?.threadId ?? 'compaction-pending');
      }
    }
    switch (message.method) {
      case 'turn/started': this.activeTurns.add(params.turn?.id ?? 'unknown-turn'); break;
      case 'turn/completed': this.activeTurns.delete(params.turn?.id); this.activeThreads.delete('compaction-pending'); break;
      case 'thread/status/changed':
        if (params.status?.type === 'active') this.activeThreads.add(params.threadId);
        else if (['idle', 'notLoaded', 'systemError'].includes(params.status?.type)) this.activeThreads.delete(params.threadId);
        else this.unknown = true;
        break;
      case 'thread/queue/changed':
        // An ID-only notification stays unverified until the bridge retrieves
        // the authoritative native queue; old unsupported engines stay fenced.
        if (!Array.isArray(params.queue ?? params.queuedSubmissions)) this.unverifiedQueues.add(params.threadId);
        else this.verifyQueue(params.threadId, params.queue ?? params.queuedSubmissions);
        break;
      case 'process/started': this.processes.add(params.processId ?? params.id ?? 'unknown-process'); break;
      case 'process/exited': this.processes.delete(params.processId ?? params.id); break;
    }
    // Every backend request must be answered before changing its authentication.
    // Unknown requests may introduce new approval/input methods in future builds.
    if (message.method && message.id !== undefined) this.pendingApprovals.add(message.id);
    if (message.method === 'serverRequest/resolved') this.pendingApprovals.delete(params.requestId);
  }
  snapshot() {
    let reason: string | null = null;
    if (!this.ready) reason = 'Codex has not finished connecting.';
    else if (this.switching) reason = 'An account switch is already in progress.';
    else if (this.unknown || this.unverifiedQueues.size) reason = 'Codex activity could not be verified. Switching is blocked.';
    else if (this.pendingWork.size || this.activeTurns.size || this.activeThreads.size || this.processes.size) reason = 'A Codex chat, subagent, or tool is still running.';
    else if (this.pendingApprovals.size) reason = 'Codex is waiting for approval or input.';
    else if (this.queuedThreads.size) reason = 'Resolve queued messages before switching accounts.';
    return { ready: this.ready, switching: this.switching, canSwitch: reason === null, reason,
      activeTurns: this.activeTurns.size, activeThreads: this.activeThreads.size, pendingWork: this.pendingWork.size, pendingApprovals: this.pendingApprovals.size, queuedThreads: this.queuedThreads.size };
  }
  acquire() { const state = this.snapshot(); if (!state.canSwitch) throw new Error(state.reason!); this.switching = true; }
}
