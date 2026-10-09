import { ChildProcess, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { PassThrough, Writable } from 'node:stream';
import { StringDecoder } from 'node:string_decoder';
import { JsonLines, JsonLineBuffer } from './protocol';
import { realpathSync } from 'node:fs';
import { win32 } from 'node:path';

export function sameExecutablePath(first: string, second: string, platform = process.platform): boolean {
  if (platform !== 'win32') return first === second;
  const canonical = (path: string) => {
    try { path = realpathSync.native(path); } catch { /* The caller also verifies a live child. */ }
    return win32.normalize(path).replace(/^\\\\\?\\/, '').toLowerCase();
  };
  return canonical(first) === canonical(second);
}

// Node has no public API for enumerating an already-running extension's children.
// Keep this compatibility hook in one place; never fall back to another process.
export function findCodexBackend(engine: string): ChildProcessWithoutNullStreams | undefined {
  const handles = (process as any)._getActiveHandles?.();
  if (!Array.isArray(handles)) return;
  const matches = handles.filter((child: any) => child instanceof ChildProcess && typeof child.spawnfile === 'string'
    && (sameExecutablePath(child.spawnfile, engine) || (engine.endsWith('.mjs') && child.spawnargs?.some((arg: string) => sameExecutablePath(arg, engine))))
    && child.spawnargs?.includes('app-server') && child.pid && child.exitCode === null && !child.killed
    && child.stdin?.writable && !child.stdin.destroyed && child.stdout?.readable && !child.stdout.destroyed);
  if (matches.length > 1) throw new Error('Multiple Codex backends are running in this extension host. Live attachment was refused.');
  return matches[0];
}

export function tapBackend(child: ChildProcessWithoutNullStreams) {
  const input = new PassThrough(); const backendInput = new PassThrough();
  const oldWrite = child.stdin.write; const oldEmit = child.stdout.emit;
  const decoder = new StringDecoder('utf8');
  let restored = false;
  const backendOutput = new Writable({ write(chunk, encoding, callback) { oldWrite.call(child.stdin, chunk, encoding, callback); } });
  const frontendOutput = new Writable({ write(chunk, _encoding, callback) { oldEmit.call(child.stdout, 'data', Buffer.from(chunk)); callback(); } });
  const frontend = new JsonLines(input, frontendOutput);
  const lines = new JsonLines(backendInput, backendOutput);
  const incoming = new JsonLineBuffer(line => {
    try { JSON.parse(line); }
    catch {
      oldEmit.call(child.stdout, 'data', Buffer.from(line + '\n'));
      lines.emit('fault', new Error('Codex protocol framing could not be verified'));
      return;
    }
    backendInput.write(line + '\n');
  }, error => lines.emit('fault', error));
  const interceptedWrite = function (this: any, chunk: any, encoding?: any, callback?: any) {
    return input.write(chunk, encoding, callback);
  } as typeof child.stdin.write;
  const interceptedEmit = function (this: any, event: string | symbol, ...args: any[]): boolean {
    if (event !== 'data') return oldEmit.call(this, event, ...args);
    incoming.push(typeof args[0] === 'string' ? args[0] : decoder.write(args[0]));
    return true;
  } as typeof child.stdout.emit;
  child.stdin.write = interceptedWrite; child.stdout.emit = interceptedEmit;
  function restore() {
    if (restored) return; restored = true;
    if (child.stdin.write === interceptedWrite) child.stdin.write = oldWrite;
    if (child.stdout.emit === interceptedEmit) child.stdout.emit = oldEmit;
    const tail = incoming.takeTail() + decoder.end();
    if (tail) oldEmit.call(child.stdout, 'data', Buffer.from(tail));
  }
  return { child, frontend, lines, restore };
}
