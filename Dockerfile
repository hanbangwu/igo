FROM rust:alpine AS backend
WORKDIR /src
RUN apk add --no-cache musl-dev
COPY Cargo.toml Cargo.lock ./
COPY igo-core igo-core
COPY igo-server igo-server
RUN cargo test --release --workspace
RUN cargo build --release -p igo-server

FROM oven/bun AS frontend
WORKDIR /src
COPY web/package.json web/bun.lock ./
RUN bun prune --production
RUN bun install --frozen-lockfile
COPY web .
RUN bun run check
RUN bun run build

FROM scratch
COPY --from=backend /src/target/release/igo-server /igo-server
COPY --from=frontend /src/dist /web/dist
ENV STATIC_DIR=/web/dist
EXPOSE 3030
USER 1000:1000
CMD ["/igo-server"]
