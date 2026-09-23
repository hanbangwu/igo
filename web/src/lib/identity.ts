const CHARS = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'
const ID_LENGTH = 8
const TOKEN_KEY = 'igo:player-token'
const NAME_KEY = 'igo:player-name'

export function roomId(): string {
  if (!window.location.hash || window.location.hash === '#') {
    const bytes = new Uint8Array(ID_LENGTH)
    crypto.getRandomValues(bytes)
    let id = ''
    for (const byte of bytes) id += CHARS[byte % CHARS.length]
    window.history.replaceState(null, '', '#' + id)
  }
  return window.location.hash.slice(1)
}

export function onRoomChange(handler: (id: string) => void): () => void {
  const listener = () => handler(roomId())
  window.addEventListener('hashchange', listener)
  return () => window.removeEventListener('hashchange', listener)
}

export function playerToken(): string {
  let token = localStorage.getItem(TOKEN_KEY)
  if (!token) {
    token = crypto.randomUUID()
    localStorage.setItem(TOKEN_KEY, token)
  }
  return token
}

export function playerName(): string {
  return localStorage.getItem(NAME_KEY) ?? randomName()
}

export function setPlayerName(name: string): void {
  localStorage.setItem(NAME_KEY, name)
}

const ADJECTIVES = ['Quiet', 'Steady', 'Patient', 'Bold', 'Clever', 'Calm', 'Sharp', 'Gentle']
const NOUNS = ['Crane', 'Turtle', 'Fox', 'Heron', 'Badger', 'Otter', 'Hare', 'Carp']

function randomName(): string {
  const pick = <T>(xs: T[]): T => xs[Math.floor(Math.random() * xs.length)]
  return `${pick(ADJECTIVES)} ${pick(NOUNS)}`
}

function apiUrl(path: string): URL {
  const base = import.meta.env.VITE_API_URL
  return new URL(path, base ? base.replace(/\/*$/, '/') : window.location.href)
}

export function socketUrl(room: string): string {
  const url = apiUrl(`api/socket/${room}`)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  return url.href
}

export function sgfUrl(room: string): string {
  return apiUrl(`api/sgf/${room}`).href
}
