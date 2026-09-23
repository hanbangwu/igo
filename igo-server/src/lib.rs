use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use dashmap::DashMap;
use igo_core::DEFAULT_KOMI;
use serde::Serialize;
use tokio::time::{self, Instant};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

pub mod protocol;
pub mod room;

use room::Room;

const MAX_ROOM_ID_LEN: usize = 32;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub max_idle: Duration,
    pub sweep_interval: Duration,
    pub board_size: u8,
    pub komi: f32,
    pub static_dir: PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            max_idle: Duration::from_secs(7 * 24 * 3600),
            sweep_interval: Duration::from_secs(3600),
            board_size: 19,
            komi: DEFAULT_KOMI,
            static_dir: PathBuf::from("web/dist"),
        }
    }
}

struct RoomEntry {
    last_accessed: Instant,
    room: Arc<Room>,
}

impl Drop for RoomEntry {
    fn drop(&mut self) {
        self.room.kill();
    }
}

#[derive(Clone)]
struct AppState {
    rooms: Arc<DashMap<String, RoomEntry>>,
    config: Arc<ServerConfig>,
    start_time: u64,
}

impl AppState {
    fn room(&self, id: &str) -> Arc<Room> {
        use dashmap::mapref::entry::Entry;

        match self.rooms.entry(id.to_owned()) {
            Entry::Occupied(mut e) => {
                let entry = e.get_mut();
                entry.last_accessed = Instant::now();
                Arc::clone(&entry.room)
            }
            Entry::Vacant(e) => {
                info!(room = id, "creating room");
                let room = Arc::new(Room::new(self.config.board_size, self.config.komi));
                e.insert(RoomEntry {
                    last_accessed: Instant::now(),
                    room: Arc::clone(&room),
                });
                room
            }
        }
    }
}

fn valid_room_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ROOM_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn app(config: ServerConfig) -> Router {
    let static_dir = config.static_dir.clone();
    let (max_idle, sweep_interval) = (config.max_idle, config.sweep_interval);

    let state = AppState {
        rooms: Arc::new(DashMap::new()),
        config: Arc::new(config),
        start_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    tokio::spawn(cleaner(state.clone(), max_idle, sweep_interval));

    let index = static_dir.join("index.html");
    let files = ServeDir::new(static_dir).fallback(ServeFile::new(index));

    Router::new()
        .route("/api/socket/{id}", get(socket_handler))
        .route("/api/sgf/{id}", get(sgf_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/health", get(|| async { "ok" }))
        .with_state(state)
        .fallback_service(files)
}

async fn socket_handler(
    Path(id): Path<String>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    if !valid_room_id(&id) {
        return (StatusCode::BAD_REQUEST, "invalid room id").into_response();
    }
    let room = state.room(&id);
    ws.on_upgrade(move |socket| async move { room.on_connection(socket).await })
}

async fn sgf_handler(Path(id): Path<String>, State(state): State<AppState>) -> Response {
    if !valid_room_id(&id) {
        return (StatusCode::BAD_REQUEST, "invalid room id").into_response();
    }
    let Some(sgf) = state.rooms.get(&id).map(|entry| entry.room.sgf()) else {
        return (StatusCode::NOT_FOUND, "no such game").into_response();
    };
    (
        [
            (header::CONTENT_TYPE, "application/x-go-sgf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{id}.sgf\""),
            ),
        ],
        sgf,
    )
        .into_response()
}

#[derive(Serialize)]
struct Stats {
    start_time: u64,
    rooms: usize,
}

async fn stats_handler(State(state): State<AppState>) -> Json<Stats> {
    Json(Stats {
        start_time: state.start_time,
        rooms: state.rooms.len(),
    })
}

async fn cleaner(state: AppState, max_idle: Duration, sweep_interval: Duration) {
    loop {
        time::sleep(sweep_interval).await;
        let stale: Vec<String> = state
            .rooms
            .iter()
            .filter(|entry| entry.last_accessed.elapsed() > max_idle)
            .map(|entry| entry.key().clone())
            .collect();
        if !stale.is_empty() {
            info!(count = stale.len(), "reclaiming idle rooms");
            for key in stale {
                state.rooms.remove(&key);
            }
        }
    }
}
