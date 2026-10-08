import { createConnection, type Socket } from 'node:net';
import { EventEmitter } from 'node:events';
import { JsonLines, RpcPeer } from './protocol';

export class BridgeClient extends EventEmitter {
  private socket: Socket;
  private peer: RpcPeer;
  state: any = null;
  private constructor(socket: Socket) {
    super(); this.socket = socket;
    const lines = new JsonLines(socket, socket); this.peer = new RpcPeer(lines, 'control:');
    lines.on('message', message => { if (message.method === 'state/changed') { this.state = message.params; this.emit('state', this.state); } });
    socket.on('close', () => this.emit('close')); socket.on('error', () => this.peer.close());
  }
  static async connect(socketPath: string, token: string) {
    const socket = createConnection(socketPath);
    await new Promise<void>((resolve, reject) => { socket.once('connect', resolve); socket.once('error', reject); });
    const client = new BridgeClient(socket);
    try { client.state = await client.peer.request('connect', { token }, 3000); return client; }
    catch (error) { client.dispose(); throw error; }
  }
  request(method: string, params: any = {}) { return this.peer.request(method, params, method === 'switch' ? 60000 : 20000); }
  dispose() { this.peer.close(); this.socket.destroy(); }
}
