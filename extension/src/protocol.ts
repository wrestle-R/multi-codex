import { EventEmitter } from 'node:events';
import { randomUUID } from 'node:crypto';
import type { Writable, Readable } from 'node:stream';

export type Message = { id?: string | number; method?: string; params?: any; result?: any; error?: { code: number; message: string } };
export class JsonLines extends EventEmitter {
  private buffer = '';
  constructor(input: Readable, private output: Writable, maxBytes = 8 * 1024 * 1024) {
    super();
    input.setEncoding('utf8');
    input.on('data', (chunk: string) => {
      this.buffer += chunk;
      let offset: number;
      while ((offset = this.buffer.indexOf('\n')) >= 0) {
        const line = this.buffer.slice(0, offset); this.buffer = this.buffer.slice(offset + 1);
        if (Buffer.byteLength(line) > maxBytes) { this.emit('fault', new Error('Protocol frame too large')); continue; }
        if (!line.trim()) continue;
        let message;
        try { message = JSON.parse(line); } catch { this.emit('fault', new Error('Invalid protocol frame')); continue; }
        if (!message || typeof message !== 'object' || Array.isArray(message)) { this.emit('fault', new Error('Invalid protocol frame')); continue; }
        this.emit('message', message);
      }
      if (Buffer.byteLength(this.buffer) > maxBytes) { this.buffer = ''; this.emit('fault', new Error('Protocol buffer too large')); }
    });
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
