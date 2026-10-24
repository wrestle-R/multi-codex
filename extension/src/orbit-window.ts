import { createServer, type Server } from 'node:net';
import { chmod, unlink } from 'node:fs/promises';

// A focus-only endpoint in the owning extension host. No workspace is opened,
// no chat is resumed and no authentication or backend RPC is exposed.
export class OrbitWindow {
  private server?: Server;
  private path?: string;
  async listen(id: string, focus: () => Promise<unknown>) {
    await this.dispose();
    if (process.platform === 'win32') return; // Orbit is supported on macOS/Linux.
    if (!/^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(id)) throw new Error('Invalid Orbit window');
    const path = `/tmp/mcx-orbit-${id}.sock`;
    const server = createServer(socket => {
      socket.setTimeout(3000, () => socket.destroy());
      socket.on('error', () => {});
      let input = ''; let requested = false;
      socket.on('data', data => {
        if (requested) return;
        input += data.toString('utf8');
        if (input.length > 128) { socket.destroy(); return; }
        if (!input.includes('\n')) return;
        requested = true;
        if (input.trim() !== id) { socket.end('closed\n'); return; }
        void focus().then(() => socket.end('focused\n'), () => socket.end('unavailable\n'));
      });
    });
    try {
      await new Promise<void>((resolve, reject) => { server.once('error', reject); server.listen(path, resolve); });
      this.server = server; this.path = path;
      await chmod(path, 0o600);
    } catch (error) { server.close(); throw error; }
  }
  async dispose() {
    const server = this.server; const path = this.path;
    this.server = undefined; this.path = undefined;
    if (server) server.close();
    if (path) await unlink(path).catch(() => {});
  }
}
