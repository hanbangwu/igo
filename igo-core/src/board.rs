use serde::{Deserialize, Serialize};

pub type Vertex = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    Black,
    White,
}

impl Color {
    #[inline]
    pub fn opposite(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }

    #[inline]
    pub fn index(self) -> usize {
        match self {
            Color::Black => 0,
            Color::White => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Point {
    #[default]
    Empty,
    Stone(Color),
}

impl Point {
    #[inline]
    pub fn as_byte(self) -> u8 {
        match self {
            Point::Empty => 0,
            Point::Stone(Color::Black) => 1,
            Point::Stone(Color::White) => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub color: Color,
    pub stones: Vec<Vertex>,
    pub liberties: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    size: u8,
    points: Vec<Point>,
}

impl Board {
    pub fn new(size: u8) -> Board {
        let size = size.clamp(2, 25);
        Board {
            size,
            points: vec![Point::Empty; size as usize * size as usize],
        }
    }

    #[inline]
    pub fn size(&self) -> u8 {
        self.size
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    #[inline]
    pub fn contains(&self, v: Vertex) -> bool {
        (v as usize) < self.points.len()
    }

    #[inline]
    pub fn vertex(&self, row: u8, col: u8) -> Option<Vertex> {
        (row < self.size && col < self.size)
            .then(|| row as Vertex * self.size as Vertex + col as Vertex)
    }

    #[inline]
    pub fn coords(&self, v: Vertex) -> (u8, u8) {
        (
            (v / self.size as Vertex) as u8,
            (v % self.size as Vertex) as u8,
        )
    }

    #[inline]
    pub fn get(&self, v: Vertex) -> Point {
        self.points.get(v as usize).copied().unwrap_or(Point::Empty)
    }

    #[inline]
    pub fn set(&mut self, v: Vertex, p: Point) {
        if let Some(slot) = self.points.get_mut(v as usize) {
            *slot = p;
        }
    }

    #[inline]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.points.iter().map(|p| p.as_byte()).collect()
    }

    pub fn vertices(&self) -> impl Iterator<Item = Vertex> {
        0..self.points.len() as Vertex
    }

    #[inline]
    pub fn neighbors(&self, v: Vertex) -> impl Iterator<Item = Vertex> {
        let size = self.size as Vertex;
        let (row, col) = (v / size, v % size);
        let mut buf = [0 as Vertex; 4];
        let mut n = 0;
        if row > 0 {
            buf[n] = v - size;
            n += 1;
        }
        if row + 1 < size {
            buf[n] = v + size;
            n += 1;
        }
        if col > 0 {
            buf[n] = v - 1;
            n += 1;
        }
        if col + 1 < size {
            buf[n] = v + 1;
            n += 1;
        }
        buf.into_iter().take(n)
    }

    pub fn group_at(&self, v: Vertex) -> Option<Group> {
        let Point::Stone(color) = self.get(v) else {
            return None;
        };
        if !self.contains(v) {
            return None;
        }

        let mut seen = vec![false; self.points.len()];
        let mut liberty_seen = vec![false; self.points.len()];
        let mut stones = Vec::new();
        let mut liberties = 0;
        let mut stack = vec![v];
        seen[v as usize] = true;

        while let Some(cur) = stack.pop() {
            stones.push(cur);
            for n in self.neighbors(cur) {
                match self.get(n) {
                    Point::Empty => {
                        if !liberty_seen[n as usize] {
                            liberty_seen[n as usize] = true;
                            liberties += 1;
                        }
                    }
                    Point::Stone(c) if c == color && !seen[n as usize] => {
                        seen[n as usize] = true;
                        stack.push(n);
                    }
                    _ => {}
                }
            }
        }

        stones.sort_unstable();
        Some(Group {
            color,
            stones,
            liberties,
        })
    }

    pub fn has_liberty(&self, v: Vertex) -> bool {
        let Point::Stone(color) = self.get(v) else {
            return false;
        };
        let mut seen = vec![false; self.points.len()];
        let mut stack = vec![v];
        seen[v as usize] = true;

        while let Some(cur) = stack.pop() {
            for n in self.neighbors(cur) {
                match self.get(n) {
                    Point::Empty => return true,
                    Point::Stone(c) if c == color && !seen[n as usize] => {
                        seen[n as usize] = true;
                        stack.push(n);
                    }
                    _ => {}
                }
            }
        }
        false
    }
}
