use axum::extract::ws::Message;
use igo_core::{Applied, Color, Move, Phase, Score, Vertex};
use serde::{Deserialize, Serialize};

pub type ConnId = u64;

pub type PlayerToken = String;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Seats {
    pub black: Option<String>,
    pub white: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello {
        token: PlayerToken,
        name: String,
    },
    ClaimSeat {
        color: Color,
    },
    ReleaseSeat,
    Play {
        move_number: usize,
        #[serde(rename = "move")]
        mv: Move,
    },
    ToggleDead {
        vertex: Vertex,
    },
    AcceptScore,
    ResumePlay,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Identity {
        conn: ConnId,
        you: Option<Color>,
    },
    Snapshot(Box<Snapshot>),
    History {
        start: usize,
        moves: Vec<Applied>,
        to_play: Color,
        captures: [u32; 2],
        phase: Phase,
    },
    Seats {
        seats: Seats,
    },
    Dead {
        vertices: Vec<Vertex>,
        accepted: Vec<Color>,
        score: Score,
        territory: Vec<u8>,
    },
    PhaseChanged {
        phase: Phase,
    },
    Presence {
        connections: usize,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub size: u8,
    pub komi: f32,
    pub board: Vec<u8>,
    pub to_play: Color,
    pub move_number: usize,
    pub captures: [u32; 2],
    pub phase: Phase,
    pub seats: Seats,
    pub dead: Vec<Vertex>,
    pub accepted: Vec<Color>,
    pub you: Option<Color>,
    pub connections: usize,
}

impl From<ServerMsg> for Message {
    fn from(msg: ServerMsg) -> Message {
        Message::text(serde_json::to_string(&msg).expect("ServerMsg is always serializable"))
    }
}
