import type { ClientMsg, ServerMsg } from "./protocol";

export type ConnectionStatus =
  | "connecting"
  | "connected"
  | "disconnected"
  | "desynchronized";

export type ConnectionOptions = {
  url: string;
  onMessage: (msg: ServerMsg) => void;
  /** Runs on every fresh socket, for re-sending the hello and seat state. */
  onOpen: () => void;
  reconnectInterval?: number;
};

/**
 * A self-healing WebSocket, ported from Rustpad's `tryConnect`.
 *
 * Reconnection is driven by a plain interval rather than backoff, because the
 * failure being handled is a dropped socket to a server that is still there —
 * a laptop waking up, a phone changing network. Repeated failures instead
 * mean something is wrong that reconnecting will not fix.
 */
export class Connection {
  status = $state<ConnectionStatus>("connecting");

  #options: ConnectionOptions;
  #ws?: WebSocket;
  #connecting = false;
  #recentFailures = 0;
  #tryConnectId: number;
  #resetFailuresId: number;
  #disposed = false;

  constructor(options: ConnectionOptions) {
    this.#options = options;
    const interval = options.reconnectInterval ?? 1000;

    this.#tryConnect();
    this.#tryConnectId = window.setInterval(() => this.#tryConnect(), interval);
    this.#resetFailuresId = window.setInterval(
      () => (this.#recentFailures = 0),
      15 * interval,
    );
  }

  /** Send a message. Returns false if there is currently no open socket. */
  send(msg: ClientMsg): boolean {
    if (this.#ws?.readyState !== WebSocket.OPEN) return false;
    this.#ws.send(JSON.stringify(msg));
    return true;
  }

  dispose(): void {
    this.#disposed = true;
    window.clearInterval(this.#tryConnectId);
    window.clearInterval(this.#resetFailuresId);
    const ws = this.#ws;
    this.#ws = undefined;
    ws?.close();
  }

  /**
   * Safety invariant: while a socket is live no other is attempted, because
   * one of `#connecting` or `#ws` is always truthy until it closes.
   *
   * Liveness invariant: once the socket closes, both are falsy again, so the
   * next interval tick will retry.
   */
  #tryConnect(): void {
    if (this.#disposed || this.#connecting || this.#ws) return;
    this.#connecting = true;

    const ws = new WebSocket(this.#options.url);

    ws.onopen = () => {
      this.#connecting = false;
      this.#ws = ws;
      this.status = "connected";
      this.#options.onOpen();
    };

    ws.onclose = () => {
      if (this.#ws === ws) {
        this.#ws = undefined;
        this.status = "disconnected";
        if (++this.#recentFailures >= 5) {
          // Five drops inside fifteen reconnect intervals is not a flaky
          // network; the client is in a state reconnecting will not repair.
          this.dispose();
          this.status = "desynchronized";
        }
      } else {
        // Never finished opening.
        this.#connecting = false;
        this.status = "disconnected";
      }
    };

    ws.onerror = () => {
      // `onclose` always follows, and does the bookkeeping.
    };

    ws.onmessage = ({ data }) => {
      if (typeof data !== "string") return;
      this.#options.onMessage(JSON.parse(data) as ServerMsg);
    };
  }

  /** Force a reconnect, used when the move log has a gap. */
  resync(): void {
    const ws = this.#ws;
    this.#ws = undefined;
    ws?.close();
  }
}
