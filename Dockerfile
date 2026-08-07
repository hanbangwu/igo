# Rust server. Tests run here so a broken ruleset cannot be released.
FROM rust:alpine AS backend
WORKDIR /src
RUN apk add --no-cache musl-dev
COPY Cargo.toml Cargo.lock ./
COPY igo-core igo-core
COPY igo-server igo-server
RUN cargo test --release --workspace
RUN cargo build --release -p igo-server

FROM node:lts-alpine AS frontend
WORKDIR /src
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web .
RUN npm run check
RUN npm run build

# Nothing but the binary and the assets: no shell, no package manager.
FROM scratch
COPY --from=backend /src/target/release/igo-server /igo-server
COPY --from=frontend /src/dist /web/dist
ENV STATIC_DIR=/web/dist
EXPOSE 3030
USER 1000:1000
CMD ["/igo-server"]
