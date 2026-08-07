use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::board::{Board, Color, Point, Vertex};
use crate::game::{Game, GameResult, Move, Phase};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SgfError {
    #[error("unexpected end of input")]
    UnexpectedEnd,
    #[error("no game tree found")]
    NoGameTree,
    #[error("board size {0} is not supported")]
    BadSize(u32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SgfGame {
    pub size: u8,
    pub komi: f32,
    pub setup_black: Vec<Vertex>,
    pub setup_white: Vec<Vertex>,
    pub moves: Vec<(Color, Move)>,
}

pub fn vertex_to_sgf(board: &Board, v: Vertex) -> String {
    let (row, col) = board.coords(v);
    let letter = |n: u8| (b'a' + n) as char;
    format!("{}{}", letter(col), letter(row))
}

pub fn sgf_to_vertex(size: u8, s: &str) -> Option<Vertex> {
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        return None; // empty value: a pass
    }
    let col = bytes[0].checked_sub(b'a')?;
    let row = bytes[1].checked_sub(b'a')?;
    if size <= 19 && col == 19 && row == 19 {
        return None;
    }
    if col >= size || row >= size {
        return None;
    }
    Some(row as Vertex * size as Vertex + col as Vertex)
}

pub fn to_sgf(game: &Game) -> String {
    let board = game.board();
    let size = board.size();

    let mut out = String::from("(;GM[1]FF[4]CA[UTF-8]AP[igo:0.1.0]RU[Chinese]");
    out.push_str(&format!("SZ[{}]KM[{}]", size, game.komi()));

    if let Phase::Finished { result } = game.phase() {
        out.push_str(&format!("RE[{}]", result_to_sgf(result)));
    }

    for applied in game.history() {
        let color = match applied.color {
            Color::Black => 'B',
            Color::White => 'W',
        };
        match applied.mv {
            Move::Play { vertex } => {
                out.push_str(&format!(";{}[{}]", color, vertex_to_sgf(board, vertex)));
            }
            Move::Pass => out.push_str(&format!(";{}[]", color)),
            Move::Resign => {}
        }
    }

    out.push(')');
    out
}

fn result_to_sgf(result: &GameResult) -> String {
    match result {
        GameResult::Resignation { winner } => match winner {
            Color::Black => "B+R".into(),
            Color::White => "W+R".into(),
        },
        GameResult::Counted { winner, score } => match winner {
            Some(Color::Black) => format!("B+{}", score.margin()),
            Some(Color::White) => format!("W+{}", -score.margin()),
            None => "0".into(),
        },
    }
}

pub fn parse(input: &str) -> Result<SgfGame, SgfError> {
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut depth = 0usize;

    let mut size: u8 = 19;
    let mut komi: f32 = crate::game::DEFAULT_KOMI;
    let mut setup_black = Vec::new();
    let mut setup_white = Vec::new();
    let mut raw_moves: Vec<(Color, String)> = Vec::new();
    let mut raw_setup: Vec<(Color, Vec<String>)> = Vec::new();
    let mut saw_tree = false;

    while i < bytes.len() {
        match bytes[i] {
            b'(' => {
                depth += 1;
                saw_tree = true;
                i += 1;
            }
            b')' => {
                depth = depth.saturating_sub(1);
                i += 1;
                if depth == 0 {
                    break;
                }
            }
            b';' | b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b if b.is_ascii_uppercase() => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_uppercase() {
                    i += 1;
                }
                let ident = &input[start..i];

                let mut values = Vec::new();
                loop {
                    while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                        i += 1;
                    }
                    if i >= bytes.len() || bytes[i] != b'[' {
                        break;
                    }
                    i += 1;
                    let mut value = String::new();
                    loop {
                        if i >= bytes.len() {
                            return Err(SgfError::UnexpectedEnd);
                        }
                        match bytes[i] {
                            b'\\' => {
                                i += 1;
                                if i < bytes.len() {
                                    value.push(bytes[i] as char);
                                    i += 1;
                                }
                            }
                            b']' => {
                                i += 1;
                                break;
                            }
                            c => {
                                value.push(c as char);
                                i += 1;
                            }
                        }
                    }
                    values.push(value);
                }

                if depth > 1 {
                    continue;
                }

                match ident {
                    "SZ" => {
                        if let Some(v) = values.first() {
                            let first = v.split(':').next().unwrap_or(v);
                            let n: u32 = first.trim().parse().unwrap_or(19);
                            if !(2..=25).contains(&n) {
                                return Err(SgfError::BadSize(n));
                            }
                            size = n as u8;
                        }
                    }
                    "KM" => {
                        if let Some(v) = values.first() {
                            komi = v.trim().parse().unwrap_or(crate::game::DEFAULT_KOMI);
                        }
                    }
                    "AB" => raw_setup.push((Color::Black, values)),
                    "AW" => raw_setup.push((Color::White, values)),
                    "B" => {
                        raw_moves.push((Color::Black, values.first().cloned().unwrap_or_default()))
                    }
                    "W" => {
                        raw_moves.push((Color::White, values.first().cloned().unwrap_or_default()))
                    }
                    _ => {}
                }
            }
            _ => i += 1,
        }
    }

    if !saw_tree {
        return Err(SgfError::NoGameTree);
    }

    for (color, values) in raw_setup {
        for v in values {
            if let Some(vertex) = sgf_to_vertex(size, &v) {
                match color {
                    Color::Black => setup_black.push(vertex),
                    Color::White => setup_white.push(vertex),
                }
            }
        }
    }

    let moves = raw_moves
        .into_iter()
        .map(|(color, v)| {
            let mv = match sgf_to_vertex(size, &v) {
                Some(vertex) => Move::Play { vertex },
                None => Move::Pass,
            };
            (color, mv)
        })
        .collect();

    Ok(SgfGame {
        size,
        komi,
        setup_black,
        setup_white,
        moves,
    })
}

pub fn replay(record: &SgfGame) -> (Game, Option<(usize, crate::game::MoveError)>) {
    let mut board = Board::new(record.size);
    for &v in &record.setup_black {
        board.set(v, Point::Stone(Color::Black));
    }
    for &v in &record.setup_white {
        board.set(v, Point::Stone(Color::White));
    }

    let first = if record.setup_black.len() > record.setup_white.len() {
        Color::White
    } else {
        Color::Black
    };
    let mut game = Game::from_board(board, first, record.komi);

    for (i, &(color, mv)) in record.moves.iter().enumerate() {
        if let Err(e) = game.replay_move(color, mv) {
            return (game, Some((i, e)));
        }
    }
    (game, None)
}
