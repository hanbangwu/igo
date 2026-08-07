/**
 * Room ids and player identity.
 *
 * The room id lives in the URL fragment and is generated in the browser, so
 * sharing a link is the only step needed to start a game — there is no
 * create-game request. The player token lives in `localStorage` and is what
 * lets a refresh reclaim your seat.
 */

const CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const ID_LENGTH = 8;
const TOKEN_KEY = "igo:player-token";
const NAME_KEY = "igo:player-name";

/** The current room id, minting one into the URL if there is none. */
export function roomId(): string {
  if (!window.location.hash || window.location.hash === "#") {
    const bytes = new Uint8Array(ID_LENGTH);
    crypto.getRandomValues(bytes);
    let id = "";
    for (const byte of bytes) id += CHARS[byte % CHARS.length];
    window.history.replaceState(null, "", "#" + id);
  }
  return window.location.hash.slice(1);
}

/** Watch for the id changing, e.g. the user pasting a different link. */
export function onRoomChange(handler: (id: string) => void): () => void {
  const listener = () => handler(roomId());
  window.addEventListener("hashchange", listener);
  return () => window.removeEventListener("hashchange", listener);
}

/**
 * A stable id for this browser.
 *
 * Seats are keyed by this rather than by connection, so refreshing the page
 * does not surrender your colour and nobody else can play your stones.
 */
export function playerToken(): string {
  let token = localStorage.getItem(TOKEN_KEY);
  if (!token) {
    token = crypto.randomUUID();
    localStorage.setItem(TOKEN_KEY, token);
  }
  return token;
}

export function playerName(): string {
  return localStorage.getItem(NAME_KEY) ?? randomName();
}

export function setPlayerName(name: string): void {
  localStorage.setItem(NAME_KEY, name);
}

const ADJECTIVES = [
  "Quiet", "Steady", "Patient", "Bold", "Clever", "Calm", "Sharp", "Gentle",
];
const NOUNS = ["Crane", "Turtle", "Fox", "Heron", "Badger", "Otter", "Hare", "Carp"];

function randomName(): string {
  const pick = <T>(xs: T[]): T => xs[Math.floor(Math.random() * xs.length)];
  return `${pick(ADJECTIVES)} ${pick(NOUNS)}`;
}

/** The WebSocket URL for a room, matching the page's scheme. */
export function socketUrl(room: string): string {
  const url = new URL(`api/socket/${room}`, window.location.href);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.href;
}
