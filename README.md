# igo

Igo (stylized as igo) is a minimal [go](<https://en.wikipedia.org/wiki/Go_(game)>) platform.

The engine is written in Rust with Chinese rules and 7.5 komi.

## Setup

```
cargo run -p igo-server               # backend on 3030
cd web && bun install && bun run dev  # frontend on 5173
```

## Deployment

```
docker build -t igo .
docker run --rm -p 3030:3030 igo
```
