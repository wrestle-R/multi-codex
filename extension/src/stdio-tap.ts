import { ChildProcess, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { PassThrough, Writable } from 'node:stream';
import { StringDecoder } from 'node:string_decoder';
import { JsonLines } from './protocol';

// Node has no public API for enumerating an already-running extension's children.
// Keep this compatibility hook in one place; never fall back to another process.
export function findCodexBackend(engine: string): ChildProcessWithoutNullStreams | undefined {
  const handles = (process as any)._getActiveHandles?.();
  if (!Array.isArray(handles)) return;
  const matches = handles.filter((child: any) => child instanceof ChildProcess && child.spawnfile === engine
    && child.spawnargs?.includes('app-server') && child.pid && child.exitCode === null && !child.killed
    && child.stdin?.writable && !child.stdin.destroyed && child.stdout?.readable && !child.stdout.destroyed);
  if (matches.length > 1) throw new Error('Multiple Codex backends are running in this extension host. Live attachment was refused.');
  return matches[0];
}

export function tapBackend(child: ChildProcessWithoutNullStreams) {
  const input = new PassThrough(); const backendInput = new PassThrough();
  const oldWrite = child.stdin.write; const oldEmit = child.stdout.emit;
  const decoder = new StringDecoder('utf8');
  let incoming = ''; let restored = false;
  const backendOutput = new Writable({ write(chunk, encoding, callback) { oldWrite.call(child.stdin, chunk, encoding, callback); } });
  const frontendOutput = new Writable({ write(chunk, _encoding, callback) { oldEmit.call(child.stdout, 'data', Buffer.from(chunk)); callback(); } });
  const frontend = new JsonLines(input, frontendOutput);
  const lines = new JsonLines(backendInput, backendOutput);
  const interceptedWrite = function (this: any, chunk: any, encoding?: any, callback?: any) {
    return input.write(chunk, encoding, callback);
  } as typeof child.stdin.write;
  const interceptedEmit = function (this: any, event: string | symbol, ...args: any[]): boolean {
    if (event !== 'data') return oldEmit.call(this, event, ...args);
    incoming += typeof args[0] === 'string' ? args[0] : decoder.write(args[0]);
    let offset: number;
    while ((offset = incoming.indexOf('\n')) >= 0) {
      const line = incoming.slice(0, offset + 1); incoming = incoming.slice(offset + 1);
      try { JSON.parse(line); }
      catch {
        // Attachment in the middle of a frame must not damage Codex's own reader.
        oldEmit.call(this, 'data', Buffer.from(line));
        lines.emit('fault', new Error('Codex protocol framing could not be verified'));
        continue;
      }
      backendInput.write(line);
    }
    if (Buffer.byteLength(incoming) > 8 * 1024 * 1024) {
      oldEmit.call(this, 'data', Buffer.from(incoming)); incoming = '';
      lines.emit('fault', new Error('Codex protocol frame is too large'));
    }
    return true;
  } as typeof child.stdout.emit;
  child.stdin.write = interceptedWrite; child.stdout.emit = interceptedEmit;
  function restore() {
    if (restored) return; restored = true;
    if (child.stdin.write === interceptedWrite) child.stdin.write = oldWrite;
    if (child.stdout.emit === interceptedEmit) child.stdout.emit = oldEmit;
    const tail = incoming + decoder.end(); incoming = '';
    if (tail) oldEmit.call(child.stdout, 'data', Buffer.from(tail));
  }
  return { child, frontend, lines, restore };
}
