//! Test harness: a server on an ephemeral port and a JSON-speaking client.

// Each test binary compiles this module separately and uses a different subset.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use igo_server::{app, ServerConfig};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

/// How long a test waits for an expected message before giving up.
const RECV_TIMEOUT: Duration = Duration::from_secs(5);

/// Start a server on a free port and return its address.
pub async fn spawn(config: ServerConfig) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app(config)).await.unwrap();
    });
    addr
}

pub async fn spawn_default() -> SocketAddr {
    spawn(ServerConfig {
        // Tests do not need the real static assets to exist.
        static_dir: "/nonexistent".into(),
        ..Default::default()
    })
    .await
}

/// A WebSocket client that speaks JSON and keeps a running view of the game,
/// the way the real browser client does.
///
/// Folding history into local state matters because the server batches: a
/// connection that is several moves behind receives them in one message. Tests
/// that assumed one message per move were reading the wrong element.
pub struct Client {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// Applied moves by move number. Entries before a snapshot are `null`,
    /// since a snapshot conveys the board rather than the log.
    moves: Vec<Value>,
    phase: Value,
    to_play: Value,
    captures: Value,
}

impl Client {
    pub async fn connect(addr: SocketAddr, room: &str) -> Client {
        let (ws, _) = connect_async(format!("ws://{addr}/api/socket/{room}"))
            .await
            .expect("websocket handshake");
        Client {
            ws,
            moves: Vec::new(),
            phase: Value::Null,
            to_play: Value::Null,
            captures: Value::Null,
        }
    }

    /// Connect and identify in one step, returning once the snapshot arrives.
    pub async fn join(addr: SocketAddr, room: &str, token: &str, name: &str) -> Client {
        let mut client = Client::connect(addr, room).await;
        client
            .send(&json!({ "type": "hello", "token": token, "name": name }))
            .await;
        client.expect("identity").await;
        client.expect("snapshot").await;
        client
    }

    pub async fn send(&mut self, msg: &Value) {
        self.ws
            .send(Message::text(msg.to_string()))
            .await
            .expect("send");
    }

    /// Receive the next JSON message, folding game state into this client.
    pub async fn recv(&mut self) -> Value {
        loop {
            let msg = tokio::time::timeout(RECV_TIMEOUT, self.ws.next())
                .await
                .expect("timed out waiting for a message")
                .expect("socket closed unexpectedly")
                .expect("websocket error");
            match msg {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text).expect("valid JSON");
                    self.absorb(&value);
                    return value;
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => panic!("server closed the connection"),
                other => panic!("unexpected frame: {other:?}"),
            }
        }
    }

    fn absorb(&mut self, msg: &Value) {
        match msg["type"].as_str() {
            Some("snapshot") => {
                let n = msg["move_number"].as_u64().unwrap_or(0) as usize;
                self.moves.resize(n, Value::Null);
                self.phase = msg["phase"].clone();
                self.to_play = msg["to_play"].clone();
                self.captures = msg["captures"].clone();
            }
            Some("history") => {
                let start = msg["start"].as_u64().unwrap_or(0) as usize;
                self.moves.resize(start, Value::Null);
                for applied in msg["moves"].as_array().into_iter().flatten() {
                    self.moves.push(applied.clone());
                }
                self.phase = msg["phase"].clone();
                self.to_play = msg["to_play"].clone();
                self.captures = msg["captures"].clone();
            }
            Some("phase_changed") => self.phase = msg["phase"].clone(),
            _ => {}
        }
    }

    /// Receive until a message of the given `type` arrives.
    pub async fn expect(&mut self, ty: &str) -> Value {
        self.expect_matching(ty, |_| true).await
    }

    /// Receive until a message of the given `type` satisfies `predicate`.
    ///
    /// Presence and seat broadcasts interleave with everything else, so tests
    /// that care about one kind of message would otherwise be order-sensitive.
    pub async fn expect_matching(
        &mut self,
        ty: &str,
        mut predicate: impl FnMut(&Value) -> bool,
    ) -> Value {
        for _ in 0..32 {
            let msg = self.recv().await;
            if msg["type"] == ty && predicate(&msg) {
                return msg;
            }
        }
        panic!("never received a matching {ty:?} message");
    }

    /// Wait until the seats reach exactly this state.
    ///
    /// Every `hello` and every claim broadcasts seats, so waiting for "the
    /// next seats message" would race. Tests use this both to assert and as a
    /// barrier before sending moves.
    pub async fn expect_seats(&mut self, black: Option<&str>, white: Option<&str>) -> Value {
        self.expect_matching("seats", |v| {
            v["seats"]["black"] == json!(black) && v["seats"]["white"] == json!(white)
        })
        .await
    }

    /// Wait until move `n` has arrived and return it.
    pub async fn expect_move(&mut self, n: usize) -> Value {
        while self.moves.len() <= n {
            self.recv().await;
        }
        self.moves[n].clone()
    }

    /// Wait until the log is at least `n` moves long.
    pub async fn sync_to(&mut self, n: usize) {
        while self.moves.len() < n {
            self.recv().await;
        }
    }

    pub fn phase(&self) -> &Value {
        &self.phase
    }

    pub fn to_play(&self) -> &Value {
        &self.to_play
    }

    pub fn captures(&self) -> &Value {
        &self.captures
    }

    pub fn move_count(&self) -> usize {
        self.moves.len()
    }

    /// Assert that nothing arrives within a short window.
    pub async fn expect_silence(&mut self) {
        let result = tokio::time::timeout(Duration::from_millis(250), self.ws.next()).await;
        if let Ok(Some(Ok(Message::Text(text)))) = result {
            panic!("expected no message, got {text}");
        }
    }

    /// Assert the server hung up.
    pub async fn expect_closed(&mut self) {
        for _ in 0..32 {
            let msg = tokio::time::timeout(RECV_TIMEOUT, self.ws.next())
                .await
                .expect("timed out waiting for close");
            match msg {
                None | Some(Ok(Message::Close(_))) | Some(Err(_)) => return,
                Some(Ok(_)) => continue,
            }
        }
        panic!("connection stayed open");
    }
}

/// Vertex index from row and column on a 19x19 board.
pub fn at(row: u16, col: u16) -> u16 {
    row * 19 + col
}

pub fn play(move_number: usize, vertex: u16) -> Value {
    json!({
        "type": "play",
        "move_number": move_number,
        "move": { "type": "play", "vertex": vertex },
    })
}

pub fn pass(move_number: usize) -> Value {
    json!({
        "type": "play",
        "move_number": move_number,
        "move": { "type": "pass" },
    })
}

pub fn claim(color: &str) -> Value {
    json!({ "type": "claim_seat", "color": color })
}
