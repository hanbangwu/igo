# igo

Two-player Go over a shared link. Open the page, send the URL to a friend, pick
colours, play. Nothing to install, no account, no database.

The rules are a Rust engine written from scratch: Chinese (area) scoring,
positional superko, suicide forbidden, komi 7.5.

## Layout

```
igo-core/     the rules. Pure Rust — no I/O, no async, no wasm bindings.
igo-server/   axum + tokio. Holds rooms in memory and is the sole authority.
web/          Vite + Svelte 5. Built to web/dist and served by the binary.
```

`igo-core` is an ordinary library crate. It compiles natively into the server
today, and the same source is meant to compile to `wasm32-unknown-unknown`
behind a thin `wasm-bindgen` wrapper so the browser can run identical rules.
Keeping it free of platform dependencies is what makes that possible later
without touching it.

## Running it

```
cargo run -p igo-server          # backend on :3030
cd web && npm install && npm run dev   # frontend on :5173, proxying /api
```

Open <http://localhost:5173>. The URL gains a random fragment like `#a7Kq2mZ1` —
that fragment *is* the game. Open it in a second window, take opposite seats,
and play.

For a single-process run against the built frontend:

```
cd web && npm run build && cd ..
STATIC_DIR=web/dist cargo run --release -p igo-server   # everything on :3030
```

### Configuration

| Variable      | Default    | Meaning                                  |
| ------------- | ---------- | ---------------------------------------- |
| `PORT`        | `3030`     | Listen port                              |
| `STATIC_DIR`  | `web/dist` | Built frontend to serve                  |
| `EXPIRY_DAYS` | `7`        | Idle days before a room is reclaimed     |
| `BOARD_SIZE`  | `19`       | Board side length                        |
| `KOMI`        | `7.5`      | Compensation for white                   |
| `RUST_LOG`    | `info`     | Log filter                               |

## Testing

```
cargo test --workspace   # rules + server
cd web && npm run check  # svelte-check
```

The rules tests are ASCII board diagrams, which makes the interesting positions
readable: captures, suicide, snapback, ko, superko, seki, dead-stone removal.
The server tests drive real WebSockets against a server on an ephemeral port.

## Deployment

```
docker build -t igo .
docker run --rm -p 3030:3030 igo
```

The image is `FROM scratch` — the binary and the built assets, nothing else. No
volume is needed because nothing is persisted.

## Design notes

**Rooms are implicit.** There is no create-game endpoint. The fragment is
generated in the browser and the room is created server-side by the first
WebSocket to arrive for that id (`igo-server/src/lib.rs`, `AppState::room`).
Idle rooms are swept by a background task; dropping the entry kills the
connections still attached to it.

**Seats belong to people, not connections.** The browser mints a token into
`localStorage` and presents it in the first message. Seats are keyed by that
token, so refreshing the page reclaims your colour and nobody else can play
your stones. Connection ids are used only for logging and for addressing
refusals.

**The move log is the game.** Everything board-related flows through an
append-only list replayed with a `start` index, so reconnect-resync, spectators
joining mid-game, and SGF export all come from one mechanism. Ephemeral state —
seats, presence, dead-stone marks — goes over a separate broadcast channel.

**Refusals are not fatal.** An illegal move gets a `rejected` message on that
one connection and the socket stays open. Only unparseable input ends a
connection. Misclicking a ko should cost nothing.

**The client has no rules engine.** Every accepted move carries the stones it
removed, so applying one in the browser is "place a stone, lift these". The
server also sends per-point territory during scoring, since the client cannot
work it out. This is what lets the wasm build be a later, purely additive step.

## Not built yet

`igo-wasm`, so the browser can validate locally and render without a round
trip; SGF import and review mode; clocks; chat; persistence across restarts.
