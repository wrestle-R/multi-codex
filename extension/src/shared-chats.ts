import { copyFile, lstat, mkdir, open, readFile, realpath, rename, rm } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import { basename, isAbsolute, join, relative, resolve } from 'node:path';
import type { Message } from './protocol';
import { ThreadOperations } from './thread-operations';

type Request = (method: string, params?: any) => Promise<any>;
export type SharedChat = { id: string; nativeId: string; sourceProfileId: string; checkpoint: string; revision: number; metadata: any; archived: boolean };
export type SharedContext = {
  helper: Request; backend: Request; home: () => string;
  owner: () => { instanceId: string; backendPid: number; sessionHome: string };
  profile: () => string | null; verifyAccount: () => Promise<void>;
};
const LOCAL_TYPES = new Set(['localImage', 'local_image', 'localAudio', 'local_audio']);
const MUTATIONS = new Set(['turn/start', 'turn/steer', 'turn/addUserMessage', 'review/start', 'thread/compact/start', 'thread/shellCommand', 'thread/queue/add', 'thread/queue/enqueue', 'thread/queue/submit', 'thread/queue/start', 'thread/realtime/start', 'thread/realtime/appendAudio', 'thread/realtime/appendText', 'thread/realtime/appendSpeech', 'command/exec', 'process/start']);
const LOCAL_MUTATIONS = new Set(['thread/settings/update', 'turn/settings/update', 'thread/metadata/update', 'thread/name/set', 'thread/queue/delete', 'thread/queue/update', 'thread/queue/reorder', 'thread/queue/remove', 'thread/queue/clear']);
const LIBRARY_MUTATIONS = new Set(['thread/archive', 'thread/unarchive', 'thread/delete']);
const READS = new Set(['thread/loaded/list', 'thread/unsubscribe', 'turn/interrupt', 'thread/realtime/stop', 'thread/queue/list']);
const SETTINGS_KEY = '_multiCodexSettings';
const SETTING_FIELDS = ['model', 'effort', 'summary', 'serviceTier'];
type LifecycleIntent = { schema: 1; id: string; nativeId: string; method: string; phase: 'prepared' | 'nativeApplied'; registered: boolean };

/** Canonical bytes are published only by a kernel-locked, verified native writer. */
export class SharedChats {
  private owned = new Map<string, { nativeId: string; path?: string; registered: boolean; metadata?: any; profileId?: string; hasWork?: boolean; settings?: Record<string, unknown> }>();
  private nativeIds = new Map<string, string>();
  private deleted = new Set<string>();
  private checkpoints = Promise.resolve();
  private catalogCursor: number | undefined;
  private known = new Set<string>();
  private observers = new Map<string, string>();
  private starting = new Set<Promise<void>>();
  private operations = new ThreadOperations();
  constructor(private context: SharedContext) {}
  get threadIds() { return [...this.owned.keys()]; }
  logical(nativeId: string) { return this.nativeIds.get(nativeId) ?? nativeId; }
  private native(logicalId: string) { return this.owned.get(logicalId)?.nativeId ?? logicalId; }
  private async acquire(id: string) {
    await this.context.helper('chats/acquire', { threadId: id, profileId: this.context.profile(), ...this.context.owner() });
  }
  private async verify(id: string, authenticate = true) {
    if (this.deleted.has(id)) throw new Error('This shared chat was deleted. Create or choose another chat.');
    await this.context.helper('chats/verify', { threadId: id });
    if (authenticate) await this.context.verifyAccount();
  }
  private thread(thread: any, id: string) { const { [SETTINGS_KEY]: _settings, ...publicThread } = thread ?? {}; return { ...publicThread, id }; }
  private settingsPath(id: string) { return join(this.context.home(), 'chat-settings', `${id}.json`); }
  private async persistSettings(id: string, params: any) {
    const owned = this.owned.get(id)!;
    const settings = { ...owned.settings };
    for (const field of SETTING_FIELDS) if (Object.hasOwn(params, field)) {
      if (params[field] === null) delete settings[field]; else settings[field] = params[field];
    }
    owned.settings = settings;
    await mkdir(join(this.context.home(), 'chat-settings'), { recursive: true, mode: 0o700 });
    const temporary = `${this.settingsPath(id)}.${randomUUID()}.tmp`;
    const file = await open(temporary, 'wx', 0o600);
    try { await file.writeFile(JSON.stringify(settings)); await file.sync(); } finally { await file.close(); }
    try { await rename(temporary, this.settingsPath(id)); } finally { await rm(temporary, { force: true }); }
  }

  async request(method: string, params: any = {}): Promise<any> {
    if (typeof params.threadId === 'string') {
      await Promise.all([...this.starting]);
      if (MUTATIONS.has(method) || LOCAL_MUTATIONS.has(method) || LIBRARY_MUTATIONS.has(method) || method === 'thread/resume' || method === 'thread/fork') {
        return this.operations.run(params.threadId, () => this.execute(method, params));
      }
    }
    return this.execute(method, params);
  }
  private async execute(method: string, params: any): Promise<any> {
    if (method === 'thread/start') await this.checkpoints;
    if (method === 'thread/list') return this.list(params);
    if (method === 'thread/read') {
      const id = params.threadId;
      if (this.owned.has(id)) {
        const empty = this.owned.get(id)!;
        const includeTurns = params.includeTurns && !empty.hasWork && !empty.registered && !await this.hasRollout(empty.path) ? false : params.includeTurns;
        const result = await this.context.backend(method, { ...params, includeTurns, threadId: this.native(id) });
        return { ...result, thread: this.thread(result.thread, id) };
      }
      return this.context.helper('chats/read', { threadId: id, includeTurns: params.includeTurns });
    }
    if (['thread/turns/list', 'thread/items/list', 'thread/attachment/list', 'thread/attachmentOwner/list', 'thread/goal/get'].includes(method)) {
      if (typeof params.threadId !== 'string') throw new Error('Choose a saved chat to read its history.');
      if (this.owned.has(params.threadId)) return this.context.backend(method, { ...params, threadId: this.native(params.threadId) });
      return this.context.helper('chats/nativeRead', { threadId: params.threadId, method, params });
    }
    if (method === 'thread/resume') return this.resume(params.threadId, params);
    if (method === 'thread/start' || method === 'thread/fork') {
      if (params.ephemeral === true) throw new Error('Shared chats require a saved history. Start a persistent chat.');
      await this.context.verifyAccount();
      if (method === 'thread/fork') { await this.ensureLoaded(params.threadId); await this.verify(params.threadId); }
      let ready!: () => void;
      const starting = new Promise<void>(resolve => { ready = resolve; });
      this.starting.add(starting);
      try {
      const result = await this.context.backend(method, { ...params, ...(method === 'thread/fork' ? { threadId: this.native(params.threadId) } : {}) });
      const id = result.thread?.id;
      if (typeof id !== 'string') throw new Error('Codex did not provide a stable new thread identity.');
      await this.acquire(id);
      this.owned.set(id, { nativeId: id, path: result.thread.path, registered: false, metadata: result.thread, profileId: this.context.profile() ?? undefined });
      this.nativeIds.set(id, id);
      // The native engine creates a rollout lazily on the first turn. An empty
      // new chat still holds its writer lease; never manufacture history bytes.
      if (method === 'thread/fork' || await this.hasRollout(result.thread.path)) await this.checkpoint(id);
      return result;
      } finally { this.starting.delete(starting); ready(); }
    }
    if (LIBRARY_MUTATIONS.has(method)) return this.lifecycle(method, params);
    if (method === 'thread/loaded/list') {
      const result = await this.context.backend(method, params);
      return { ...result, data: result.data.map((id: string) => this.logical(id)) };
    }
    if (MUTATIONS.has(method)) {
      if (typeof params.threadId !== 'string') throw new Error('Shared work requires an identified chat.');
      await this.ensureLoaded(params.threadId); await this.verify(params.threadId);
      const prepared = await this.copyInputAssets(params);
      this.owned.get(params.threadId)!.hasWork = true;
      const result = await this.context.backend(method, { ...prepared, threadId: this.native(params.threadId) });
      return result;
    }
    if (LOCAL_MUTATIONS.has(method)) {
      if (typeof params.threadId !== 'string') throw new Error('Choose a saved chat before changing its settings.');
      await this.ensureLoaded(params.threadId, false); await this.verify(params.threadId, false);
      const result = await this.context.backend(method, { ...params, threadId: this.native(params.threadId) });
      if (method === 'thread/settings/update') await this.persistSettings(params.threadId, params);
      const current = await this.context.backend('thread/read', { threadId: this.native(params.threadId), includeTurns: false });
      const owned = this.owned.get(params.threadId)!;
      owned.metadata = current.thread; owned.path = current.thread?.path ?? owned.path;
      if (['idle', 'notLoaded'].includes(current.thread?.status?.type) && await this.hasRollout(owned.path)) await this.checkpoint(params.threadId, false, true);
      return result.thread ? { ...result, thread: this.thread(result.thread, params.threadId) } : result;
    }
    if (READS.has(method)) {
      if (typeof params.threadId === 'string') {
        if (!this.owned.has(params.threadId)) {
          if (method === 'thread/queue/list') {
            await this.context.helper('chats/get', { threadId: params.threadId });
            return { data: [], nextCursor: null };
          }
          throw new Error('This window does not own the shared chat.');
        }
        // Cancellation remains possible after token expiry; it cannot start a
        // model request or transfer this window's writer ownership.
        await this.context.helper('chats/verify', { threadId: params.threadId });
        params = { ...params, threadId: this.native(params.threadId) };
      }
      return this.context.backend(method, params);
    }
    // A new upstream mutation must receive an explicit ownership route first.
    if (method.startsWith('thread/') || method.startsWith('turn/') || method.startsWith('review/')) {
      throw new Error('This Codex operation has not been verified for shared chats. The original history is preserved.');
    }
    if (['account/login/start', 'account/logout', 'login', 'logout'].includes(method)) {
      throw new Error('Use Multi Codex to sign in or choose an account for this shared window.');
    }
    return this.context.backend(method, params);
  }

  private intentPath(id: string) {
    if (!/^[A-Za-z0-9-]+$/.test(id)) throw new Error('Invalid chat operation identity');
    return join(this.context.home(), 'chat-operations', `${id}.json`);
  }
  private async saveIntent(intent: LifecycleIntent) {
    const path = this.intentPath(intent.id);
    await mkdir(join(this.context.home(), 'chat-operations'), { recursive: true, mode: 0o700 });
    const temp = `${path}.${randomUUID()}.tmp`;
    const file = await open(temp, 'wx', 0o600);
    try { await file.writeFile(JSON.stringify(intent)); await file.sync(); }
    finally { await file.close(); }
    try {
      await rename(temp, path);
      if (process.platform !== 'win32') {
        const directory = await open(join(this.context.home(), 'chat-operations'), 'r');
        try { await directory.sync(); } finally { await directory.close(); }
      }
    } finally { await rm(temp, { force: true }); }
  }
  private async lifecycle(method: string, params: any) {
    const id = params.threadId;
    if (typeof id !== 'string') throw new Error('Choose a saved chat before changing its library state.');
    let intent: LifecycleIntent | undefined;
    try {
      const path = this.intentPath(id);
      const info = await lstat(path);
      if (!info.isFile() || info.isSymbolicLink()) throw new Error('Chat operation recovery cannot follow links.');
      intent = JSON.parse(await readFile(path, 'utf8'));
      if (intent?.schema !== 1 || intent.id !== id || typeof intent.nativeId !== 'string' || !['prepared', 'nativeApplied'].includes(intent.phase) || typeof intent.registered !== 'boolean' || !LIBRARY_MUTATIONS.has(intent.method)) throw new Error('Chat operation recovery is invalid.');
      if (intent.method !== method) throw new Error('Retry the unfinished library operation before making another change.');
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
    await this.ensureLoaded(id, false); await this.verify(id, false);
    const owned = this.owned.get(id)!;
    if (intent && intent.nativeId !== owned.nativeId) throw new Error('Chat operation recovery has a different native identity.');
    if (!intent) {
      const state = await this.context.backend('thread/read', { threadId: owned.nativeId, includeTurns: false });
      if (!['idle', 'notLoaded'].includes(state.thread?.status?.type)) throw new Error('Wait for this chat and its tools to finish before changing its library state.');
      if (await this.hasRollout(state.thread?.path ?? owned.path)) await this.checkpoint(id, false);
      else if (owned.registered || owned.hasWork) throw new Error('This chat has unverified history. Keep the window open for recovery.');
      intent = { schema: 1, id, nativeId: owned.nativeId, method, phase: 'prepared', registered: owned.registered };
      await this.saveIntent(intent);
    }
    let result: any = {};
    if (intent.phase === 'prepared') {
      result = await this.context.backend(method, { ...params, threadId: intent.nativeId });
      intent.phase = 'nativeApplied'; await this.saveIntent(intent);
    } else if (method === 'thread/unarchive') {
      result = await this.context.backend('thread/read', { threadId: intent.nativeId, includeTurns: false });
    }
    if (method !== 'thread/delete') await this.checkpoint(id, false);
    if (method !== 'thread/delete' || intent.registered) await this.context.helper(`chats/${method.split('/')[1]}`, { threadId: id });
    if (method === 'thread/delete') this.deleted.add(id);
    await rm(this.intentPath(id));
    return result.thread ? { ...result, thread: this.thread(result.thread, id) } : result;
  }

  private async list(params: any) {
    const filter = JSON.stringify({ archived: !!params.archived, cwd: params.cwd ?? null, searchTerm: params.searchTerm ?? null, sourceKinds: params.sourceKinds ?? [], modelProviders: params.modelProviders ?? [] });
    let offset = 0;
    if (params.cursor) {
      try { const cursor = JSON.parse(Buffer.from(params.cursor, 'base64url').toString());
        if (cursor.filter !== filter || !Number.isSafeInteger(cursor.offset) || cursor.offset < 0) throw new Error();
        offset = cursor.offset;
      } catch { throw new Error('Shared chat pagination changed. Refresh the chat list.'); }
    }
    const limit = Math.min(100, Math.max(1, params.limit ?? 50));
    const data: any[] = [];
    let exhausted = false;
    while (data.length <= limit && !exhausted) {
      const page: SharedChat[] = await this.context.helper('chats/list', { archived: !!params.archived, offset, limit: 100 });
      if (!page.length) { exhausted = true; break; }
      for (const saved of page) {
        const thread = saved.metadata;
        const provider = thread.modelProvider;
        const subagent = thread.source?.subAgent;
        const source = typeof thread.source === 'string' ? thread.source : subagent === 'review' ? 'subAgentReview' : subagent === 'compact' ? 'subAgentCompact' : subagent?.thread_spawn ? 'subAgentThreadSpawn' : subagent ? 'subAgentOther' : 'unknown';
        if ((!params.cwd || thread.cwd === params.cwd)
          && (!params.searchTerm || `${thread.name ?? ''} ${thread.preview ?? thread.snippet ?? ''}`.toLowerCase().includes(String(params.searchTerm).toLowerCase()))
          && (!params.sourceKinds?.length || params.sourceKinds.includes(source) || source?.startsWith('subAgent') && params.sourceKinds.includes('subAgent'))
          && (!params.modelProviders?.length || params.modelProviders.includes(provider))) {
          if (data.length === limit) return { data, nextCursor: Buffer.from(JSON.stringify({ offset, filter })).toString('base64url') };
          const owned = this.owned.get(saved.id);
          this.known.add(saved.id);
          data.push({ ...this.thread(thread, saved.id), id: saved.id, turns: [], status: owned?.metadata?.status ?? { type: owned ? 'idle' : 'notLoaded' } });
        }
        offset++;
      }
      exhausted = page.length < 100;
    }
    return { data, nextCursor: null };
  }

  async catalogEvents(): Promise<Message[]> {
    const response = await this.context.helper('chats/changes', { cursor: this.catalogCursor ?? null });
    if (!Number.isSafeInteger(response.cursor) || !Array.isArray(response.data)) throw new Error('The shared catalog could not provide verified change events.');
    this.catalogCursor = response.cursor;
    const events: Message[] = [];
    if (response.reset) {
      for (const threadId of this.known) {
        if (this.owned.has(threadId)) continue;
        try {
          const saved: SharedChat = await this.context.helper('chats/get', { threadId });
          events.push({ method: saved.archived ? 'thread/archived' : 'thread/unarchived', params: { threadId } });
        } catch { /* Retain the UI entry when current catalog state is unverified. */ }
      }
    }
    for (const change of response.data) {
      const threadId = change.threadId;
      if (typeof threadId !== 'string' || this.owned.has(threadId)) continue;
      if (change.kind === 'updated') {
        if (!this.known.has(threadId)) {
          this.known.add(threadId);
          events.push({ method: 'thread/started', params: { thread: { ...change.metadata, id: threadId, turns: [], status: { type: 'notLoaded' } } } });
        }
        events.push({ method: 'thread/name/updated', params: { threadId, threadName: change.metadata.name ?? null } });
        events.push({ method: 'thread/status/changed', params: { threadId, status: { type: 'notLoaded' } } });
      } else if (['archived', 'unarchived', 'deleted'].includes(change.kind)) {
        events.push({ method: `thread/${change.kind}`, params: { threadId } });
        if (change.kind === 'deleted') this.known.delete(threadId);
      }
    }
    return events;
  }

  private async ensureLoaded(id: string, authenticate = true) {
    if (!this.owned.has(id)) await this.resume(id, { threadId: id }, authenticate);
    if (!this.owned.has(id)) throw new Error(this.observers.get(id) ?? 'Another window owns this shared chat. Release it there before continuing.');
  }
  private async resume(id: string, params: any, authenticate = true) {
    if (authenticate) await this.context.verifyAccount();
    if (!this.owned.has(id)) {
      const saved: SharedChat = await this.context.helper('chats/get', { threadId: id });
      const conflict = this.nativeIds.get(saved.nativeId);
      if (conflict && conflict !== id) throw new Error('Two imported chats have the same native identity. Open the other chat in a separate managed window.');
      try { await this.acquire(id); }
      catch (error) {
        const reason = error instanceof Error ? error.message : '';
        if (!/another window|previous native backend|previous backend|stopped writer/.test(reason)) throw error;
        this.observers.set(id, reason);
        this.known.add(id);
        return this.context.helper('chats/observe', { threadId: id, includeTurns: true });
      }
      const hydrated = await this.context.helper('chats/hydrate', { threadId: id });
      this.owned.set(id, { nativeId: saved.nativeId, path: hydrated.path, registered: true, settings: saved.metadata?.[SETTINGS_KEY] });
      this.nativeIds.set(saved.nativeId, id);
      this.observers.delete(id);
    }
    await this.verify(id, authenticate);
    const saved = this.owned.get(id)!;
    const preferences = saved.settings ?? {};
    const restored = { model: preferences.model, serviceTier: preferences.serviceTier };
    const config = { ...(Object.hasOwn(preferences, 'effort') ? { model_reasoning_effort: preferences.effort } : {}), ...(Object.hasOwn(preferences, 'summary') ? { model_reasoning_summary: preferences.summary } : {}), ...params.config };
    const result = await this.context.backend('thread/resume', { ...restored, ...params, config, threadId: saved.nativeId, path: saved.path });
    saved.path = result.thread?.path ?? saved.path;
    saved.metadata = result.thread;
    return { ...result, thread: this.thread(result.thread, id) };
  }

  async checkpoint(id: string, authenticate = false, deferBusy = false) {
    if (this.deleted.has(id)) return;
    const owned = this.owned.get(id); if (!owned) return;
    await this.verify(id, authenticate);
    const result = await this.context.backend('thread/read', { threadId: owned.nativeId, includeTurns: false });
    if (!result.thread || !['idle', 'notLoaded'].includes(result.thread.status?.type)) {
      if (deferBusy && result.thread) return;
      throw new Error('Wait for this chat and its tools to finish before publishing or handing it off.');
    }
    const path = result.thread.path ?? owned.path;
    owned.metadata = result.thread; owned.path = path;
    if (!path) throw new Error('Codex has not persisted a complete rollout for this chat.');
    const assets = await this.assets(path);
    const stable = await this.context.backend('thread/read', { threadId: owned.nativeId, includeTurns: false });
    if (!['idle', 'notLoaded'].includes(stable.thread?.status?.type)) {
      if (deferBusy) return;
      throw new Error('Chat work resumed during publication. Wait for it to finish before handing it off.');
    }
    const profileId = this.context.profile();
    if (!profileId) throw new Error('Choose a saved account before publishing a shared chat.');
    const committed = await this.context.helper(owned.registered ? 'chats/checkpoint' : 'chats/register', { threadId: id, profileId, path, metadata: { ...result.thread, [SETTINGS_KEY]: owned.settings }, assets });
    if (committed.id !== id) throw new Error('Shared chat identity changed during publication. Reopen this window before continuing.');
    owned.path = path; owned.registered = true;
  }
  private async hasRollout(path: unknown) {
    if (typeof path !== 'string') return false;
    try { await lstat(path); return true; }
    catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false; throw error; }
  }
  async captureForStop() {
    await this.checkpoints;
    await this.adoptLoaded();
    for (const [id, owned] of this.owned) {
      if (this.deleted.has(id)) continue;
      const result = await this.context.backend('thread/read', { threadId: owned.nativeId, includeTurns: false });
      owned.metadata = result.thread; owned.path = result.thread?.path ?? owned.path;
      owned.profileId ??= this.context.profile() ?? undefined;
    }
  }
  async publishAfterStop() {
    await this.checkpoints;
    for (const [id, owned] of this.owned) {
      if (this.deleted.has(id)) continue;
      if (!owned.registered && !await this.hasRollout(owned.path)) {
        if (owned.hasWork || !owned.metadata?.turns || owned.metadata.turns.length !== 0) throw new Error('A new chat has unverified history. Its private home and owner have been retained.');
        continue;
      }
      if (!owned.path || !owned.metadata) throw new Error('Private chat recovery is required before releasing this writer.');
      if (!['idle', 'notLoaded'].includes(owned.metadata.status?.type)) throw new Error('Stopped chat work is incomplete. Its private history and ownership are retained for explicit recovery.');
      await this.context.helper(owned.registered ? 'chats/checkpointStopped' : 'chats/registerStopped', {
        threadId: id, profileId: owned.profileId ?? this.context.profile(), path: owned.path,
        metadata: { ...owned.metadata, [SETTINGS_KEY]: owned.settings, status: { type: 'notLoaded' } }, assets: await this.assets(owned.path),
      });
    }
  }
  queueCheckpoint(id: string, failed: (error: unknown) => void) {
    this.checkpoints = this.checkpoints.then(async () => {
      await this.adoptLoaded();
      await this.operations.run(id, () => this.checkpoint(id, false, true));
    }).catch(failed);
  }
  async drain() {
    await this.operations.drain();
    await this.checkpoints;
    await this.adoptLoaded();
    for (const [id, owned] of this.owned) {
      if (this.deleted.has(id)) continue;
      if (!owned.registered && !await this.hasRollout(owned.path)) {
        if (owned.hasWork || owned.metadata?.turns?.length !== 0) throw new Error('A new chat has not persisted its history. Keep this window open and retry.');
        continue;
      }
      await this.checkpoint(id);
    }
  }
  private async adoptLoaded() {
    const loaded = await this.context.backend('thread/loaded/list', {});
    if (!Array.isArray(loaded.data)) throw new Error('Loaded shared chats could not be inventoried.');
    for (const nativeId of loaded.data) {
      if (this.nativeIds.has(nativeId)) continue;
      const result = await this.context.backend('thread/read', { threadId: nativeId, includeTurns: false });
      if (!result.thread?.path) throw new Error('A loaded child chat has no recoverable rollout.');
      await this.acquire(nativeId);
      this.owned.set(nativeId, { nativeId, path: result.thread.path, registered: false, metadata: result.thread, profileId: this.context.profile() ?? undefined, hasWork: true });
      this.nativeIds.set(nativeId, nativeId);
    }
  }
  async releaseAfterStop() {
    await this.checkpoints;
    for (const id of this.owned.keys()) await this.context.helper('chats/release', { threadId: id });
    this.owned.clear(); this.nativeIds.clear(); this.deleted.clear();
  }

  private async copyInputAssets(params: any) {
    if (!Array.isArray(params.input)) return params;
    const input = await Promise.all(params.input.map(async (item: any) => {
      if (!LOCAL_TYPES.has(item.type) || typeof item.path !== 'string') return item;
      const metadata = await lstat(item.path);
      if (!metadata.isFile() || metadata.isSymbolicLink()) throw new Error('Choose a regular local attachment file.');
      const bytes = await readFile(item.path);
      const directory = join(this.context.home(), 'attachments'); await mkdir(directory, { recursive: true, mode: 0o700 });
      const target = join(directory, createHash('sha256').update(bytes).digest('hex') + '-' + basename(item.path));
      await copyFile(item.path, target);
      if (createHash('sha256').update(await readFile(target)).digest('hex') !== createHash('sha256').update(bytes).digest('hex')) throw new Error('The attachment changed while being copied. Choose it again.');
      return { ...item, path: target };
    }));
    return { ...params, input };
  }
  private async assets(path: string): Promise<[string, string][]> {
    const home = await realpath(this.context.home());
    const inside = (candidate: string) => { const part = relative(home, candidate); return part && part !== '..' && !part.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) && !isAbsolute(part); };
    const assets = new Map<string, string>();
    const visit = async (value: any): Promise<void> => {
      if (!value || typeof value !== 'object') return;
      let reference: string | undefined;
      if (LOCAL_TYPES.has(value.type)) reference = value.path;
      else if (value.type === 'input_image') reference = value.image_url;
      else if (['image', 'audio'].includes(value.type)) reference = value.url;
      if (typeof reference === 'string' && !/^(?:https?:|data:)/i.test(reference)) {
        let file = reference.startsWith('file:') ? (await import('node:url')).fileURLToPath(reference) : resolve(home, reference);
        file = await realpath(file);
        if (!inside(file)) throw new Error('This chat contains an attachment outside its managed home. Its history remains private until that attachment is safely imported.');
        assets.set(reference, file);
      }
      for (const child of Object.values(value)) {
        if (Array.isArray(child)) for (const entry of child) await visit(entry);
        else if (child && typeof child === 'object') await visit(child);
      }
    };
    for (const line of (await readFile(path, 'utf8')).split('\n')) if (line.trim()) await visit(JSON.parse(line));
    return [...assets];
  }
  notification(message: Message): Message {
    if (!message.params) return message;
    const params = { ...message.params };
    if (typeof params.threadId === 'string') params.threadId = this.logical(params.threadId);
    if (message.method === 'thread/status/changed' && params.threadId && params.status) {
      const owned = this.owned.get(params.threadId);
      if (owned) owned.metadata = { ...owned.metadata, status: params.status };
    }
    if (params.thread?.id) params.thread = this.thread(params.thread, this.logical(params.thread.id));
    return { ...message, params };
  }
}
