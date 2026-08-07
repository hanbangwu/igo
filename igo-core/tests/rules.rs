//! Rules tests driven by ASCII board diagrams.
//!
//! `X` is black, `O` is white, `.` is empty. Row 0 is the top line.

use std::collections::HashSet;

use igo_core::board::Point;
use igo_core::{Board, Color, Game, Move, MoveError, Vertex, DEFAULT_KOMI};

/// Build a board from a diagram. Panics on a malformed diagram, which in a
/// test is exactly what you want.
fn board(size: u8, diagram: &str) -> Board {
    let mut b = Board::new(size);
    let mut row = 0u8;
    for line in diagram.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        for (col, ch) in line.chars().enumerate() {
            let v = b.vertex(row, col as u8).unwrap_or_else(|| {
                panic!("diagram cell ({row},{col}) is off a {size}x{size} board")
            });
            match ch {
                'X' => b.set(v, Point::Stone(Color::Black)),
                'O' => b.set(v, Point::Stone(Color::White)),
                '.' => {}
                other => panic!("unexpected character {other:?} in diagram"),
            }
        }
        row += 1;
    }
    b
}

fn game(size: u8, diagram: &str, to_play: Color) -> Game {
    Game::from_board(board(size, diagram), to_play, DEFAULT_KOMI)
}

/// Vertex from (row, col) for a given board size.
fn at(size: u8, row: u8, col: u8) -> Vertex {
    row as Vertex * size as Vertex + col as Vertex
}

fn play(g: &mut Game, color: Color, row: u8, col: u8) -> Result<igo_core::Applied, MoveError> {
    let size = g.board().size();
    g.play(
        color,
        Move::Play {
            vertex: at(size, row, col),
        },
    )
}

// ---------------------------------------------------------------- captures

#[test]
fn captures_a_single_stone() {
    let mut g = game(
        5,
        "
        .X...
        XO...
        .X...
        .....
        .....",
        Color::Black,
    );

    let applied = play(&mut g, Color::Black, 1, 2).expect("legal");
    assert_eq!(applied.captured, vec![at(5, 1, 1)]);
    assert_eq!(g.board().get(at(5, 1, 1)), Point::Empty);
    assert_eq!(g.captures(), [1, 0]);
}

#[test]
fn captures_a_multi_stone_group() {
    let mut g = game(
        5,
        "
        .XX..
        XOO..
        .XX..
        .....
        .....",
        Color::Black,
    );

    let applied = play(&mut g, Color::Black, 1, 3).expect("legal");
    let mut captured = applied.captured.clone();
    captured.sort_unstable();
    assert_eq!(captured, vec![at(5, 1, 1), at(5, 1, 2)]);
    assert_eq!(g.captures(), [2, 0]);
}

#[test]
fn a_group_touched_twice_is_captured_once() {
    // Black's move touches the white group at two separate points. The
    // captured list must not contain duplicates, or the client delta would
    // remove the same stone twice and the capture count would be wrong.
    let mut g = game(
        5,
        "
        .XX..
        XOOX.
        XO.X.
        .XX..
        .....",
        Color::Black,
    );

    let applied = play(&mut g, Color::Black, 2, 2).expect("legal");
    let mut captured = applied.captured.clone();
    captured.sort_unstable();
    assert_eq!(
        captured,
        vec![at(5, 1, 1), at(5, 1, 2), at(5, 2, 1)],
        "three white stones, each listed once"
    );
    assert_eq!(g.captures(), [3, 0]);
}

// ----------------------------------------------------------------- suicide

#[test]
fn suicide_is_rejected() {
    let mut g = game(
        5,
        "
        .X...
        X.X..
        .X...
        .....
        .....",
        Color::White,
    );

    assert_eq!(play(&mut g, Color::White, 1, 1), Err(MoveError::Suicide));
    assert_eq!(g.move_number(), 0, "a rejected move leaves no trace");
    assert_eq!(g.to_play(), Color::White, "and does not pass the turn");
}

#[test]
fn a_move_that_captures_is_not_suicide() {
    // Black's stone at (0,0) would have no liberties of its own, but playing
    // it removes the white group first. Resolving captures before the suicide
    // test is what makes this legal.
    let mut g = game(
        5,
        "
        .OX..
        OOX..
        XXX..
        .....
        .....",
        Color::Black,
    );

    let applied = play(&mut g, Color::Black, 0, 0).expect("captures, so not suicide");
    assert_eq!(applied.captured.len(), 3);
    assert_eq!(g.board().get(at(5, 0, 0)), Point::Stone(Color::Black));
}

#[test]
fn snapback() {
    // A white ring around a two-point eye space. Black plays inside, white
    // captures the lone stone, and that capture puts the whole ring in atari
    // so black takes all of it back.
    let mut g = game(
        6,
        "
        .XXXX.
        XOOOOX
        XO..OX
        XOOOOX
        .XXXX.
        ......",
        Color::Black,
    );

    let first = play(&mut g, Color::Black, 2, 2).expect("one liberty at (2,3), so legal");
    assert!(first.captured.is_empty(), "white still has a liberty");

    let white = play(&mut g, Color::White, 2, 3).expect("captures the intruder");
    assert_eq!(white.captured, vec![at(6, 2, 2)]);

    let recapture = play(&mut g, Color::Black, 2, 2).expect("the ring is now in atari");
    assert_eq!(recapture.captured.len(), 11, "the entire white ring");
    assert_eq!(g.captures(), [11, 1]);
}

// ---------------------------------------------------------------------- ko

const KO: &str = "
    .....
    .XO..
    X.XO.
    .XO..
    .....";

#[test]
fn ko_recapture_is_rejected() {
    let mut g = game(5, KO, Color::White);

    let take = play(&mut g, Color::White, 2, 1).expect("captures the black stone");
    assert_eq!(take.captured, vec![at(5, 2, 2)]);

    // Recapturing immediately would restore the starting position.
    assert_eq!(play(&mut g, Color::Black, 2, 2), Err(MoveError::Superko));
    assert_eq!(g.move_number(), 1);
}

#[test]
fn ko_can_be_retaken_after_a_threat_is_answered() {
    let mut g = game(5, KO, Color::White);

    play(&mut g, Color::White, 2, 1).expect("white takes the ko");
    play(&mut g, Color::Black, 4, 4).expect("black plays a threat");
    play(&mut g, Color::White, 4, 0).expect("white answers");

    // The board now differs from the original by two stones, so the same
    // recapture is a new position and therefore legal.
    let retake = play(&mut g, Color::Black, 2, 2).expect("no longer a repeat");
    assert_eq!(retake.captured, vec![at(5, 2, 1)]);
}

#[test]
fn superko_compares_against_every_previous_position() {
    // The rejected recapture above restores the *initial* position, two plies
    // back, not merely the immediately preceding one. This is positional
    // superko rather than a single remembered ko point, so longer cycles
    // (triple ko, eternal life) are caught by the same mechanism.
    let mut g = game(5, KO, Color::White);
    let start = g.board().clone();

    play(&mut g, Color::White, 2, 1).expect("white takes the ko");
    assert_ne!(g.board(), &start);

    assert_eq!(play(&mut g, Color::Black, 2, 2), Err(MoveError::Superko));
}

// ------------------------------------------------------- groups and edges

#[test]
fn corner_and_edge_liberties() {
    let b = board(
        5,
        "
        X....
        .....
        ....O
        .....
        XX..O",
        // corner, edge, and a two-stone corner group
    );

    assert_eq!(
        b.group_at(at(5, 0, 0)).unwrap().liberties,
        2,
        "corner stone"
    );
    assert_eq!(b.group_at(at(5, 2, 4)).unwrap().liberties, 3, "edge stone");

    let corner_group = b.group_at(at(5, 4, 0)).unwrap();
    assert_eq!(corner_group.stones, vec![at(5, 4, 0), at(5, 4, 1)]);
    assert_eq!(corner_group.liberties, 3, "two-stone group in the corner");

    assert!(
        b.group_at(at(5, 1, 1)).is_none(),
        "empty point has no group"
    );
}

#[test]
fn shared_liberties_are_counted_once() {
    // An L of three stones. (1,1) is adjacent to two of them but is a single
    // liberty; double-counting it would make dead groups look alive.
    let b = board(
        5,
        "
        XX...
        X....
        .....
        .....
        .....",
    );
    let g = b.group_at(at(5, 0, 0)).unwrap();
    assert_eq!(g.stones, vec![at(5, 0, 0), at(5, 0, 1), at(5, 1, 0)]);
    assert_eq!(
        g.liberties, 3,
        "(0,2), (1,1) and (2,0) — (1,1) counted once"
    );
}

// ----------------------------------------------------------- turn and flow

#[test]
fn players_must_alternate() {
    let mut g = Game::new(19, DEFAULT_KOMI);
    play(&mut g, Color::Black, 3, 3).expect("black opens");
    assert_eq!(
        play(&mut g, Color::Black, 15, 15),
        Err(MoveError::WrongTurn)
    );
    play(&mut g, Color::White, 15, 15).expect("white replies");
}

#[test]
fn occupied_and_off_board_are_rejected() {
    let mut g = Game::new(9, DEFAULT_KOMI);
    play(&mut g, Color::Black, 4, 4).expect("legal");
    assert_eq!(play(&mut g, Color::White, 4, 4), Err(MoveError::Occupied));
    assert_eq!(
        g.play(Color::White, Move::Play { vertex: 9999 }),
        Err(MoveError::OffBoard)
    );
}

#[test]
fn two_passes_enter_scoring() {
    let mut g = Game::new(9, DEFAULT_KOMI);
    g.play(Color::Black, Move::Pass).unwrap();
    assert_eq!(
        g.phase(),
        &igo_core::Phase::Playing,
        "one pass is not enough"
    );
    g.play(Color::White, Move::Pass).unwrap();
    assert_eq!(g.phase(), &igo_core::Phase::Scoring);

    // No further moves until the players resolve scoring.
    assert_eq!(play(&mut g, Color::Black, 0, 0), Err(MoveError::NotPlaying));
}

#[test]
fn a_move_between_passes_resets_the_count() {
    let mut g = Game::new(9, DEFAULT_KOMI);
    g.play(Color::Black, Move::Pass).unwrap();
    play(&mut g, Color::White, 4, 4).unwrap();
    g.play(Color::Black, Move::Pass).unwrap();
    assert_eq!(
        g.phase(),
        &igo_core::Phase::Playing,
        "passes were not consecutive"
    );
}

#[test]
fn resuming_play_leaves_the_scoring_phase() {
    let mut g = Game::new(9, DEFAULT_KOMI);
    g.play(Color::Black, Move::Pass).unwrap();
    g.play(Color::White, Move::Pass).unwrap();
    g.resume_play().expect("players disagreed on dead stones");
    assert_eq!(g.phase(), &igo_core::Phase::Playing);
    play(&mut g, Color::Black, 4, 4).expect("play continues");
}

#[test]
fn resignation_ends_the_game() {
    let mut g = Game::new(19, DEFAULT_KOMI);
    g.play(Color::Black, Move::Resign).unwrap();
    assert_eq!(
        g.phase(),
        &igo_core::Phase::Finished {
            result: igo_core::GameResult::Resignation {
                winner: Color::White
            }
        }
    );
    assert!(g.is_finished());
    assert_eq!(play(&mut g, Color::White, 0, 0), Err(MoveError::NotPlaying));
}

// ----------------------------------------------------------------- scoring

#[test]
fn area_score_counts_stones_and_territory() {
    let g = game(
        5,
        "
        .XOO.
        .XOO.
        .XOO.
        .XOO.
        .XOO.",
        Color::Black,
    );

    let score = g.provisional_score(&HashSet::new());
    assert_eq!(score.black_stones, 5);
    assert_eq!(score.black_territory, 5, "the empty left column");
    assert_eq!(score.white_stones, 10);
    assert_eq!(score.white_territory, 5, "the empty right column");
    assert_eq!(score.dame, 0);

    assert_eq!(score.black_total(), 10.0);
    assert_eq!(
        score.white_total(),
        15.0 + DEFAULT_KOMI,
        "komi goes to white"
    );
    assert_eq!(score.winner(), Some(Color::White));
}

#[test]
fn empty_points_touching_both_colours_are_dame() {
    let g = game(
        5,
        "
        .X.O.
        .X.O.
        .X.O.
        .X.O.
        .X.O.",
        Color::Black,
    );

    let score = g.provisional_score(&HashSet::new());
    assert_eq!(score.dame, 5, "the middle column touches both colours");
    assert_eq!(score.black_territory, 5);
    assert_eq!(score.white_territory, 5);
    assert_eq!(score.black_total(), 10.0);
    assert_eq!(score.white_total(), 10.0 + DEFAULT_KOMI);
}

#[test]
fn dead_stones_become_territory_for_the_surrounding_player() {
    let mut g = game(
        5,
        "
        XXXXX
        X...X
        X.O.X
        X...X
        XXXXX",
        Color::Black,
    );

    let live = g.provisional_score(&HashSet::new());
    assert_eq!(
        live.dame, 8,
        "while the white stone stands, the region is neutral"
    );
    assert_eq!(live.white_stones, 1);

    let dead = HashSet::from([at(5, 2, 2)]);
    let counted = g.provisional_score(&dead);
    assert_eq!(counted.black_stones, 16);
    assert_eq!(counted.black_territory, 9, "the whole enclosed region");
    assert_eq!(counted.white_stones, 0);
    assert_eq!(counted.dame, 0);

    g.play(Color::Black, Move::Pass).unwrap();
    g.play(Color::White, Move::Pass).unwrap();
    let final_score = g.finish_with_dead(&dead).unwrap();
    assert_eq!(final_score.winner(), Some(Color::Black));
    assert!(g.is_finished());
}

#[test]
fn seki_shared_liberties_score_for_neither_side() {
    // A real seki: the two empty points are the *only* liberties of both the
    // black and the white group. Whoever fills one goes down to a single
    // liberty and is captured, so neither can move and both live.
    //
    // Under area scoring this needs no special case at all — the region
    // touches both colours, so it is dame and counts for neither side. That is
    // the main reason to prefer area scoring over territory scoring here.
    let g = game(
        5,
        "
        XXXOO
        XX.OO
        XX.OO
        XXXOO
        XXXOO",
        Color::Black,
    );

    let black = g.board().group_at(at(5, 0, 0)).unwrap();
    let white = g.board().group_at(at(5, 0, 4)).unwrap();
    assert_eq!(
        black.liberties, 2,
        "black's only liberties are the shared pair"
    );
    assert_eq!(
        white.liberties, 2,
        "and they are white's only liberties too"
    );

    let score = g.provisional_score(&HashSet::new());
    assert_eq!(
        score.dame, 2,
        "the shared liberties belong to neither player"
    );
    assert_eq!(score.black_stones, 13);
    assert_eq!(score.white_stones, 10);
    assert_eq!(score.black_territory, 0);
    assert_eq!(score.white_territory, 0);
    assert_eq!(score.winner(), Some(Color::White), "13 against 10 + komi");
}

#[test]
fn jigo_is_possible_with_integer_komi() {
    let mut g = Game::from_board(
        board(
            4,
            "
            XXOO
            XXOO
            XXOO
            XXOO",
        ),
        Color::Black,
        0.0,
    );
    let score = g.provisional_score(&HashSet::new());
    assert_eq!(score.black_total(), score.white_total());
    assert_eq!(score.winner(), None, "a draw");

    g.play(Color::Black, Move::Pass).unwrap();
    g.play(Color::White, Move::Pass).unwrap();
    let result = g.finish_with_dead(&HashSet::new()).unwrap();
    assert_eq!(result.winner(), None);
}
