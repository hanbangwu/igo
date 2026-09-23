import type { ClientMsg, ServerMsg } from './protocol'

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'desynchronized'

export type ConnectionOptions = {
  url: string
  onMessage: (msg: ServerMsg) => void
  onOpen: () => void
  reconnectInterval?: number
}

export class Connection {
  status = $state<ConnectionStatus>('connecting')

  #options: ConnectionOptions
  #ws?: WebSocket
  #connecting = false
  #recentFailures = 0
  #tryConnectId: number
  #resetFailuresId: number
  #disposed = false

  constructor(options: ConnectionOptions) {
    this.#options = options
    const interval = options.reconnectInterval ?? 1000

    this.#tryConnect()
    this.#tryConnectId = window.setInterval(() => this.#tryConnect(), interval)
    this.#resetFailuresId = window.setInterval(() => (this.#recentFailures = 0), 15 * interval)
  }

  send(msg: ClientMsg): boolean {
    if (this.#ws?.readyState !== WebSocket.OPEN) return false
    this.#ws.send(JSON.stringify(msg))
    return true
  }

  dispose(): void {
    this.#disposed = true
    window.clearInterval(this.#tryConnectId)
    window.clearInterval(this.#resetFailuresId)
    const ws = this.#ws
    this.#ws = undefined
    ws?.close()
  }

  #tryConnect(): void {
    if (this.#disposed || this.#connecting || this.#ws) return
    this.#connecting = true

    const ws = new WebSocket(this.#options.url)

    ws.onopen = () => {
      this.#connecting = false
      this.#ws = ws
      this.status = 'connected'
      this.#options.onOpen()
    }

    ws.onclose = () => {
      if (this.#ws === ws) {
        this.#ws = undefined
        this.status = 'disconnected'
        if (++this.#recentFailures >= 5) {
          this.dispose()
          this.status = 'desynchronized'
        }
      } else {
        this.#connecting = false
        this.status = 'disconnected'
      }
    }

    ws.onerror = () => {}

    ws.onmessage = ({ data }) => {
      if (typeof data !== 'string') return
      this.#options.onMessage(JSON.parse(data) as ServerMsg)
    }
  }

  resync(): void {
    const ws = this.#ws
    this.#ws = undefined
    ws?.close()
  }
}
