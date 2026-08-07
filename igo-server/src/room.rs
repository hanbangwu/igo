use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::ws::{Message, WebSocket};
use igo_core::{Color, Game, Phase, Vertex};
use tokio::sync::{broadcast, Notify};
use tracing::{debug, info, warn};

use crate::protocol::{ClientMsg, ConnId, PlayerToken, Seats, ServerMsg, Snapshot};

const HELLO_TIMEOUT: Duration = Duration::from_secs(30);

const BROADCAST_CAPACITY: usize = 64;

const MAX_NAME_LEN: usize = 40;

struct SeatHolder {
    token: PlayerToken,
    name: String,
}

struct RoomState {
    game: Game,
    seats: [Option<SeatHolder>; 2],
    conns: HashMap<ConnId, PlayerToken>,
    names: HashMap<PlayerToken, String>,
    dead: HashSet<Vertex>,
    accepted: HashSet<Color>,
}

impl RoomState {
    fn seat_of(&self, token: &str) -> Option<Color> {
        for color in [Color::Black, Color::White] {
            if let Some(holder) = &self.seats[color.index()] {
                if holder.token == token {
                    return Some(color);
                }
            }
        }
        None
    }

    fn seats_view(&self) -> Seats {
        Seats {
            black: self.seats[Color::Black.index()]
                .as_ref()
                .map(|h| h.name.clone()),
            white: self.seats[Color::White.index()]
                .as_ref()
                .map(|h| h.name.clone()),
        }
    }

    fn dead_sorted(&self) -> Vec<Vertex> {
        let mut v: Vec<_> = self.dead.iter().copied().collect();
        v.sort_unstable();
        v
    }

    fn accepted_sorted(&self) -> Vec<Color> {
        [Color::Black, Color::White]
            .into_iter()
            .filter(|c| self.accepted.contains(c))
            .collect()
    }

    fn dead_message(&self) -> ServerMsg {
        ServerMsg::Dead {
            vertices: self.dead_sorted(),
            accepted: self.accepted_sorted(),
            score: self.game.provisional_score(&self.dead),
            territory: igo_core::score::ownership(self.game.board(), &self.dead),
        }
    }
}

pub struct Room {
    state: RwLock<RoomState>,
    notify: Notify,
    update: broadcast::Sender<ServerMsg>,
    killed: AtomicBool,
    next_conn: AtomicU64,
}

impl Room {
    pub fn new(size: u8, komi: f32) -> Room {
        let (update, _) = broadcast::channel(BROADCAST_CAPACITY);
        Room {
            state: RwLock::new(RoomState {
                game: Game::new(size, komi),
                seats: [None, None],
                conns: HashMap::new(),
                names: HashMap::new(),
                dead: HashSet::new(),
                accepted: HashSet::new(),
            }),
            notify: Notify::new(),
            update,
            killed: AtomicBool::new(false),
            next_conn: AtomicU64::new(0),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, RoomState> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, RoomState> {
        self.state.write().unwrap_or_else(|e| e.into_inner())
    }

    pub fn revision(&self) -> usize {
        self.read().game.move_number()
    }

    pub fn sgf(&self) -> String {
        igo_core::sgf::to_sgf(&self.read().game)
    }

    pub fn kill(&self) {
        self.killed.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    pub fn killed(&self) -> bool {
        self.killed.load(Ordering::Relaxed)
    }

    pub async fn on_connection(&self, socket: WebSocket) {
        let id = self.next_conn.fetch_add(1, Ordering::Relaxed);
        debug!(conn = id, "connected");

        if let Err(e) = self.handle(id, socket).await {
            debug!(conn = id, error = %e, "connection ended");
        }

        let departed = self.write().conns.remove(&id).is_some();
        debug!(conn = id, "disconnected");
        if departed {
            // Seats deliberately survive the disconnect; only presence changes.
            let connections = self.read().conns.len();
            let _ = self.update.send(ServerMsg::Presence { connections });
        }
    }

    async fn handle(&self, id: ConnId, mut socket: WebSocket) -> Result<()> {
        let mut update_rx = self.update.subscribe();

        let token = self.await_hello(id, &mut socket).await?;

        let mut you = self.read().seat_of(&token);
        socket
            .send(ServerMsg::Identity { conn: id, you }.into())
            .await?;

        let snapshot = self.snapshot_for(&token);
        socket
            .send(ServerMsg::Snapshot(Box::new(snapshot)).into())
            .await?;

        let connections = self.read().conns.len();
        let _ = self.update.send(ServerMsg::Presence { connections });

        let mut revision = self.revision();

        loop {
            let notified = self.notify.notified();
            if self.killed() {
                break;
            }
            if self.revision() > revision {
                revision = self.send_history(revision, &mut socket).await?;
            }

            tokio::select! {
                _ = notified => {}
                update = update_rx.recv() => {
                    match update {
                        Ok(msg) => {
                            if matches!(msg, ServerMsg::Seats { .. }) {
                                let now = self.read().seat_of(&token);
                                if now != you {
                                    you = now;
                                    socket
                                        .send(ServerMsg::Identity { conn: id, you }.into())
                                        .await?;
                                }
                            }
                            socket.send(msg.into()).await?;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(conn = id, skipped = n, "broadcast lagged; resyncing");
                            let snapshot = self.snapshot_for(&token);
                            socket.send(ServerMsg::Snapshot(Box::new(snapshot)).into()).await?;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                incoming = socket.recv() => {
                    match incoming {
                        None => break,
                        Some(message) => {
                            self.handle_message(id, message?, &mut socket).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn await_hello(&self, id: ConnId, socket: &mut WebSocket) -> Result<PlayerToken> {
        let deadline = tokio::time::timeout(HELLO_TIMEOUT, async {
            loop {
                match socket.recv().await {
                    None => bail!("closed before identifying itself"),
                    Some(message) => {
                        if let Message::Text(text) = message? {
                            return Ok(text);
                        }
                    }
                }
            }
        });

        let text = deadline
            .await
            .map_err(|_| anyhow!("timed out waiting for hello"))??;

        let msg: ClientMsg = serde_json::from_str(&text).context("malformed hello")?;
        let ClientMsg::Hello { token, name } = msg else {
            bail!("first message must be hello");
        };
        if token.is_empty() || token.len() > 64 {
            bail!("invalid player token");
        }

        self.set_name(Some(id), &token, name);
        Ok(token)
    }

    fn set_name(&self, conn: Option<ConnId>, token: &str, name: String) {
        let name: String = name.chars().take(MAX_NAME_LEN).collect();
        let name = if name.trim().is_empty() {
            "Anonymous".to_string()
        } else {
            name
        };

        let mut state = self.write();
        if let Some(id) = conn {
            state.conns.insert(id, token.to_owned());
        }
        state.names.insert(token.to_owned(), name.clone());
        if let Some(color) = state.seat_of(token) {
            if let Some(holder) = state.seats[color.index()].as_mut() {
                holder.name = name;
            }
        }
        let seats = state.seats_view();
        drop(state);

        let _ = self.update.send(ServerMsg::Seats { seats });
    }

    fn snapshot_for(&self, token: &str) -> Snapshot {
        let state = self.read();
        Snapshot {
            size: state.game.board().size(),
            komi: state.game.komi(),
            board: state.game.board().as_bytes(),
            to_play: state.game.to_play(),
            move_number: state.game.move_number(),
            captures: state.game.captures(),
            phase: state.game.phase().clone(),
            seats: state.seats_view(),
            dead: state.dead_sorted(),
            accepted: state.accepted_sorted(),
            you: state.seat_of(token),
            connections: state.conns.len(),
        }
    }

    async fn send_history(&self, start: usize, socket: &mut WebSocket) -> Result<usize> {
        let (msg, end) = {
            let state = self.read();
            let log = state.game.history();
            if start >= log.len() {
                return Ok(start);
            }
            let moves = log[start..].to_vec();
            let end = start + moves.len();
            (
                ServerMsg::History {
                    start,
                    moves,
                    to_play: state.game.to_play(),
                    captures: state.game.captures(),
                    phase: state.game.phase().clone(),
                },
                end,
            )
        };
        socket.send(msg.into()).await?;
        Ok(end)
    }

    async fn handle_message(
        &self,
        id: ConnId,
        message: Message,
        socket: &mut WebSocket,
    ) -> Result<()> {
        let Message::Text(text) = message else {
            return Ok(());
        };
        let msg: ClientMsg = serde_json::from_str(&text).context("malformed message")?;

        if let Err(reason) = self.apply(id, msg) {
            debug!(conn = id, %reason, "refused");
            socket.send(ServerMsg::Rejected { reason }.into()).await?;
        }
        Ok(())
    }

    fn apply(&self, id: ConnId, msg: ClientMsg) -> Result<(), String> {
        let token = self
            .read()
            .conns
            .get(&id)
            .cloned()
            .ok_or("connection has not identified itself")?;

        match msg {
            ClientMsg::Hello {
                token: sent,
                name: new_name,
            } => {
                if sent != token {
                    return Err("cannot change identity on an open connection".into());
                }
                self.set_name(None, &token, new_name);
                Ok(())
            }

            ClientMsg::ClaimSeat { color } => {
                let mut state = self.write();
                if let Some(held) = state.seat_of(&token) {
                    return Err(if held == color {
                        "you already have that seat".into()
                    } else {
                        "you are already playing the other colour".into()
                    });
                }
                if state.seats[color.index()].is_some() {
                    return Err("that seat is taken".into());
                }
                let name = state
                    .names
                    .get(&token)
                    .cloned()
                    .unwrap_or_else(|| "Anonymous".into());
                state.seats[color.index()] = Some(SeatHolder {
                    token: token.clone(),
                    name,
                });
                let seats = state.seats_view();
                drop(state);

                info!(?color, "seat claimed");
                let _ = self.update.send(ServerMsg::Seats { seats });
                Ok(())
            }

            ClientMsg::ReleaseSeat => {
                let mut state = self.write();
                let color = state.seat_of(&token).ok_or("you do not have a seat")?;
                if state.game.move_number() > 0 {
                    return Err("the game has already started".into());
                }
                state.seats[color.index()] = None;
                let seats = state.seats_view();
                drop(state);

                let _ = self.update.send(ServerMsg::Seats { seats });
                Ok(())
            }

            ClientMsg::Play { move_number, mv } => {
                let mut state = self.write();
                let color = state
                    .seat_of(&token)
                    .ok_or("you are watching this game, not playing it")?;

                let current = state.game.move_number();
                if move_number != current {
                    return Err(format!(
                        "stale move: sent at move {move_number}, game is at {current}"
                    ));
                }

                state.game.play(color, mv).map_err(|e| e.to_string())?;
                let entered_scoring = matches!(state.game.phase(), Phase::Scoring);
                let dead_msg = entered_scoring.then(|| state.dead_message());
                drop(state);

                self.notify.notify_waiters();
                if let Some(msg) = dead_msg {
                    let _ = self.update.send(msg);
                }
                Ok(())
            }

            ClientMsg::ToggleDead { vertex } => {
                let mut state = self.write();
                if !matches!(state.game.phase(), Phase::Scoring) {
                    return Err("the game is not being scored".into());
                }
                state
                    .seat_of(&token)
                    .ok_or("only the players may mark stones")?;

                let group = state
                    .game
                    .board()
                    .group_at(vertex)
                    .ok_or("there is no stone there")?;

                let all_dead = group.stones.iter().all(|v| state.dead.contains(v));
                for v in &group.stones {
                    if all_dead {
                        state.dead.remove(v);
                    } else {
                        state.dead.insert(*v);
                    }
                }
                state.accepted.clear();
                let msg = state.dead_message();
                drop(state);

                let _ = self.update.send(msg);
                Ok(())
            }

            ClientMsg::AcceptScore => {
                let mut state = self.write();
                if !matches!(state.game.phase(), Phase::Scoring) {
                    return Err("the game is not being scored".into());
                }
                let color = state
                    .seat_of(&token)
                    .ok_or("only the players may agree the score")?;
                if state.seats[Color::Black.index()].is_none()
                    || state.seats[Color::White.index()].is_none()
                {
                    return Err("both seats must be filled to finish the game".into());
                }

                state.accepted.insert(color);
                let both_agreed = state.accepted.len() == 2;

                let msgs = if both_agreed {
                    let dead = state.dead.clone();
                    state
                        .game
                        .finish_with_dead(&dead)
                        .map_err(|e| e.to_string())?;
                    vec![
                        state.dead_message(),
                        ServerMsg::PhaseChanged {
                            phase: state.game.phase().clone(),
                        },
                    ]
                } else {
                    vec![state.dead_message()]
                };
                drop(state);

                for msg in msgs {
                    let _ = self.update.send(msg);
                }
                Ok(())
            }

            ClientMsg::ResumePlay => {
                let mut state = self.write();
                state
                    .seat_of(&token)
                    .ok_or("only the players may resume the game")?;
                state.game.resume_play().map_err(|e| e.to_string())?;
                state.dead.clear();
                state.accepted.clear();
                let phase = state.game.phase().clone();
                let dead_msg = state.dead_message();
                drop(state);

                let _ = self.update.send(ServerMsg::PhaseChanged { phase });
                let _ = self.update.send(dead_msg);
                Ok(())
            }
        }
    }
}
