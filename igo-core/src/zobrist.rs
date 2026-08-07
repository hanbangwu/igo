use crate::board::{Color, Vertex};

const MAX_VERTICES: usize = 625;

const SEED: u64 = 0x00_6967_6F5F_7632; // "igo_v2"

const fn splitmix64(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31), state)
}

const fn build_table() -> [[u64; 2]; MAX_VERTICES] {
    let mut table = [[0u64; 2]; MAX_VERTICES];
    let mut state = SEED;
    let mut i = 0;
    while i < MAX_VERTICES {
        let (black, s) = splitmix64(state);
        state = s;
        let (white, s) = splitmix64(state);
        state = s;
        table[i] = [black, white];
        i += 1;
    }
    table
}

static TABLE: [[u64; 2]; MAX_VERTICES] = build_table();

#[inline]
pub fn stone(v: Vertex, color: Color) -> u64 {
    TABLE
        .get(v as usize)
        .map(|entry| entry[color.index()])
        .unwrap_or(0)
}

pub fn hash_board(board: &crate::board::Board) -> u64 {
    use crate::board::Point;
    let mut h = 0u64;
    for v in board.vertices() {
        if let Point::Stone(c) = board.get(v) {
            h ^= stone(v, c);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Point};

    #[test]
    fn table_entries_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for entry in TABLE.iter() {
            assert!(seen.insert(entry[0]), "duplicate zobrist value");
            assert!(seen.insert(entry[1]), "duplicate zobrist value");
        }
    }

    #[test]
    fn xor_is_self_inverse() {
        let h = stone(42, Color::Black);
        assert_eq!(h ^ h, 0);
        assert_ne!(stone(42, Color::Black), stone(42, Color::White));
        assert_ne!(stone(42, Color::Black), stone(43, Color::Black));
    }

    #[test]
    fn incremental_matches_full_hash() {
        let mut board = Board::new(19);
        let mut h = 0u64;
        for (v, color) in [(0, Color::Black), (20, Color::White), (360, Color::Black)] {
            board.set(v, Point::Stone(color));
            h ^= stone(v, color);
        }
        assert_eq!(h, hash_board(&board));

        board.set(20, Point::Empty);
        h ^= stone(20, Color::White);
        assert_eq!(h, hash_board(&board));
    }
}
