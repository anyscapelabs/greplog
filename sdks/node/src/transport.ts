import * as net from 'net';
import * as os from 'os';
import { encodeLengthPrefixedFrame } from './serialize';

let warningShown = false;

function showWarning(): void {
  try {
    if (warningShown) return;
    warningShown = true;
    process.stderr.write('[Greplog] Agent not found. Run \'greplog dev\' to capture logs.\n');
  } catch {
    // fail-open
  }
}

export class Transport {
  private socket: net.Socket | null = null;
  private connecting = false;
  private destroyed = false;
  private writeBuffer: Buffer[] = [];
  private flushing = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly socketPath: string;
  private readonly tcpHost: string;
  private readonly tcpPort: number;

  constructor(opts?: { socketPath?: string; tcpHost?: string; tcpPort?: number }) {
    this.socketPath = opts?.socketPath ?? '.greplog/greplog.sock';
    this.tcpHost = opts?.tcpHost ?? '127.0.0.1';
    this.tcpPort = opts?.tcpPort ?? 4318;
  }

  connect(): void {
    if (this.socket || this.connecting || this.destroyed) return;
    this.connecting = true;

    const isWindows = os.platform() === 'win32';
    const opts: net.NetConnectOpts = isWindows
      ? { host: this.tcpHost, port: this.tcpPort }
      : { path: this.socketPath };

    const sock = net.createConnection(opts);

    sock.on('connect', () => {
      if (this.destroyed) { sock.destroy(); return; }
      this.socket = sock;
      this.connecting = false;
      this.flushBuffer();
    });

    sock.on('error', (_err: Error) => {
      if (this.destroyed) { sock.destroy(); return; }

      // Attempt TCP fallback if UDS failed and we haven't already
      if (!isWindows && !opts.hasOwnProperty('host')) {
        sock.destroy();
        this.connecting = false;
        const tcpSock = net.createConnection({ host: this.tcpHost, port: this.tcpPort });
        tcpSock.on('connect', () => {
          if (this.destroyed) { tcpSock.destroy(); return; }
          this.socket = tcpSock;
          this.flushBuffer();
        });
        tcpSock.on('error', () => {
          if (this.destroyed) { tcpSock.destroy(); return; }
          tcpSock.destroy();
          this.scheduleReconnect();
        });
        tcpSock.on('close', () => {
          if (this.destroyed) return;
          if (this.socket === tcpSock) {
            this.socket = null;
            this.scheduleReconnect();
          }
        });
        return;
      }

      sock.destroy();
      this.connecting = false;
      this.scheduleReconnect();
    });

    sock.on('close', () => {
      if (this.destroyed) return;
      if (this.socket === sock) {
        this.socket = null;
        this.connecting = false;
        this.scheduleReconnect();
      }
    });

    sock.on('timeout', () => {
      sock.destroy();
    });

    sock.setTimeout(5000);
  }

  send(payload: Uint8Array): void {
    if (this.destroyed) return;

    if (!this.socket) {
      this.writeBuffer.push(encodeLengthPrefixedFrame(payload));
      if (this.writeBuffer.length > 1000) {
        this.writeBuffer.shift();
      }
      showWarning();
      this.connect();
      return;
    }

    const frame = encodeLengthPrefixedFrame(payload);
    const canContinue = this.socket.write(frame);
    if (!canContinue) {
      this.writeBuffer.push(frame);
      if (this.writeBuffer.length > 1000) {
        this.writeBuffer.shift();
      }
    }
  }

  private flushBuffer(): void {
    if (this.flushing || !this.socket || this.writeBuffer.length === 0) return;
    this.flushing = true;

    const drained = this.socket.write(Buffer.concat(this.writeBuffer));
    this.writeBuffer = [];
    this.flushing = false;
  }

  private scheduleReconnect(): void {
    if (this.destroyed || this.reconnectTimer) return;
    if (!warningShown) {
      showWarning();
    }
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, 5000);
  }

  destroy(): void {
    this.destroyed = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
    }
    this.writeBuffer = [];
  }
}
