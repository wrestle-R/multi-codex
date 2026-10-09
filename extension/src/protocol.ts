import { EventEmitter } from 'node:events';
import { randomUUID } from 'node:crypto';
import type { Writable, Readable } from 'node:stream';

export type Message = { id?: string | number; method?: string; params?: any; result?: any; error?: { code: number; message: string } };
export const MAX_CODEX_PROTOCOL_BYTES = 128 * 1024 * 1024;

export class JsonLineBuffer {
  private chunks: string[] = [];
  private bytes = 0;
  private discarding = false;
  constructor(private complete: (line: string) => void, private fault: (error: Error) => void,
    private maxBytes = MAX_CODEX_PROTOCOL_BYTES) {}
  push(chunk: string) {
    let offset = 0;
    while (offset < chunk.length) {
      const newline = chunk.indexOf('\n', offset);
      const part = chunk.slice(offset, newline < 0 ? chunk.length : newline);
      if (!this.discarding) {
        this.bytes += Buffer.byteLength(part);
        if (this.bytes > this.maxBytes) {
          this.chunks = []; this.bytes = 0; this.discarding = true;
          this.fault(new Error('Protocol frame too large'));
        } else this.chunks.push(part);
      }
      if (newline >= 0) {
        const line = this.discarding ? undefined : this.chunks.join('');
        this.chunks = []; this.bytes = 0; this.discarding = false;
        if (line?.trim()) this.complete(line);
      }
      offset = newline < 0 ? chunk.length : newline + 1;
    }
  }
  takeTail() { const tail = this.chunks.join(''); this.chunks = []; this.bytes = 0; return tail; }
}
export class JsonLines extends EventEmitter {
  constructor(input: Readable, private output: Writable, maxBytes = MAX_CODEX_PROTOCOL_BYTES) {
    super();
    input.setEncoding('utf8');
    const buffer = new JsonLineBuffer(line => {
      let message;
      try { message = JSON.parse(line); } catch { this.emit('fault', new Error('Invalid protocol frame')); return; }
      if (!message || typeof message !== 'object' || Array.isArray(message)) { this.emit('fault', new Error('Invalid protocol frame')); return; }
      this.emit('message', message);
    }, error => this.emit('fault', error), maxBytes);
    input.on('data', (chunk: string) => buffer.push(chunk));
    input.on('end', () => this.emit('end'));
    input.on('error', () => this.emit('fault', new Error('Protocol connection failed')));
    output.on('error', () => this.emit('fault', new Error('Protocol write failed')));
  }
  send(message: Message) { this.output.write(JSON.stringify(message) + '\n'); }
}

export class RpcPeer {
  private pending = new Map<string | number, { resolve: (result: any) => void; reject: (error: Error) => void; timer: NodeJS.Timeout }>();
  constructor(readonly lines: JsonLines, private prefix = 'multi-codex:') {
    lines.on('message', (message: Message) => {
      if (message.method || message.id === undefined) return;
      const pending = this.pending.get(message.id); if (!pending) return;
      this.pending.delete(message.id); clearTimeout(pending.timer);
      if (message.error) pending.reject(new Error(message.error.message)); else pending.resolve(message.result);
    });
    lines.on('end', () => this.close()); lines.on('fault', () => this.close());
  }
  request(method: string, params: any = {}, timeoutMs = 20000): Promise<any> {
    const id = this.prefix + randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`Request timed out: ${method}`)); }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer }); this.lines.send({ id, method, params });
    });
  }
  owns(id: string | number | undefined) { return typeof id === 'string' && id.startsWith(this.prefix); }
  close() { for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(new Error('Backend connection closed')); } this.pending.clear(); }
}
