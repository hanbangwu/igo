use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::board::{Board, Color, Point, Vertex};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub black_stones: u32,
    pub black_territory: u32,
    pub white_stones: u32,
    pub white_territory: u32,
    pub dame: u32,
    pub komi: f32,
}

impl Score {
    pub fn black_total(&self) -> f32 {
        (self.black_stones + self.black_territory) as f32
    }

    pub fn white_total(&self) -> f32 {
        (self.white_stones + self.white_territory) as f32 + self.komi
    }

    pub fn margin(&self) -> f32 {
        self.black_total() - self.white_total()
    }

    pub fn winner(&self) -> Option<Color> {
        let margin = self.margin();
        if margin > 0.0 {
            Some(Color::Black)
        } else if margin < 0.0 {
            Some(Color::White)
        } else {
            None
        }
    }
}

pub fn area_score(board: &Board, dead: &HashSet<Vertex>, komi: f32) -> Score {
    count(board, dead, komi).0
}

pub fn ownership(board: &Board, dead: &HashSet<Vertex>) -> Vec<u8> {
    count(board, dead, 0.0).1
}

fn count(board: &Board, dead: &HashSet<Vertex>, komi: f32) -> (Score, Vec<u8>) {
    let mut owner = vec![0u8; board.len()];
    let mut board = board.clone();
    for &v in dead {
        board.set(v, Point::Empty);
    }

    let mut stones = [0u32; 2];
    for v in board.vertices() {
        if let Point::Stone(c) = board.get(v) {
            stones[c.index()] += 1;
            owner[v as usize] = c.index() as u8 + 1;
        }
    }

    let mut territory = [0u32; 2];
    let mut dame = 0u32;
    let mut visited = vec![false; board.len()];

    for start in board.vertices() {
        if visited[start as usize] || board.get(start) != Point::Empty {
            continue;
        }

        let mut region = vec![start];
        let mut borders = [false; 2];
        let mut stack = vec![start];
        visited[start as usize] = true;

        while let Some(cur) = stack.pop() {
            for n in board.neighbors(cur) {
                match board.get(n) {
                    Point::Empty => {
                        if !visited[n as usize] {
                            visited[n as usize] = true;
                            region.push(n);
                            stack.push(n);
                        }
                    }
                    Point::Stone(c) => borders[c.index()] = true,
                }
            }
        }

        let size = region.len() as u32;
        let claimed = match (borders[Color::Black.index()], borders[Color::White.index()]) {
            (true, false) => Some(Color::Black),
            (false, true) => Some(Color::White),
            _ => None,
        };
        match claimed {
            Some(color) => {
                territory[color.index()] += size;
                for v in region {
                    owner[v as usize] = color.index() as u8 + 1;
                }
            }
            None => dame += size,
        }
    }

    let score = Score {
        black_stones: stones[Color::Black.index()],
        black_territory: territory[Color::Black.index()],
        white_stones: stones[Color::White.index()],
        white_territory: territory[Color::White.index()],
        dame,
        komi,
    };
    (score, owner)
}
