//! SGF export, parsing, and round-tripping.

use igo_core::sgf::{self, SgfGame};
use igo_core::{Color, Game, Move, DEFAULT_KOMI};

fn at(size: u8, row: u8, col: u8) -> u16 {
    row as u16 * size as u16 + col as u16
}

#[test]
fn coordinates_map_to_sgf_letters() {
    // aa is the top left; the column letter comes first.
    assert_eq!(sgf::sgf_to_vertex(19, "aa"), Some(0));
    assert_eq!(sgf::sgf_to_vertex(19, "pd"), Some(at(19, 3, 15)));
    assert_eq!(sgf::sgf_to_vertex(19, "ss"), Some(at(19, 18, 18)));

    assert_eq!(sgf::sgf_to_vertex(19, ""), None, "empty value is a pass");
    assert_eq!(
        sgf::sgf_to_vertex(19, "tt"),
        None,
        "tt is the old pass encoding"
    );
    assert_eq!(sgf::sgf_to_vertex(9, "ss"), None, "off a 9x9 board");

    // On boards larger than 19, tt is a real point rather than a pass.
    assert_eq!(sgf::sgf_to_vertex(21, "tt"), Some(at(21, 19, 19)));
}

#[test]
fn exports_a_played_game() {
    let mut game = Game::new(19, DEFAULT_KOMI);
    game.play(
        Color::Black,
        Move::Play {
            vertex: at(19, 3, 15),
        },
    )
    .unwrap();
    game.play(
        Color::White,
        Move::Play {
            vertex: at(19, 15, 3),
        },
    )
    .unwrap();
    game.play(Color::Black, Move::Pass).unwrap();

    let text = sgf::to_sgf(&game);
    assert!(text.starts_with("(;GM[1]FF[4]"), "{text}");
    assert!(text.contains("SZ[19]"));
    assert!(text.contains("KM[7.5]"));
    assert!(text.contains("RU[Chinese]"));
    assert!(text.ends_with(";B[pd];W[dp];B[])"), "{text}");
}

#[test]
fn records_the_result_of_a_finished_game() {
    let mut game = Game::new(19, DEFAULT_KOMI);
    game.play(Color::Black, Move::Resign).unwrap();
    assert!(sgf::to_sgf(&game).contains("RE[W+R]"));
}

#[test]
fn parses_size_komi_handicap_and_variations() {
    let text = "(;GM[1]FF[4]SZ[9]KM[0.5]HA[2]AB[cc][gg]\
                 ;W[ee];B[ge]\
                 (;W[gd];B[fd])\
                 (;W[fd]))";

    let record = sgf::parse(text).unwrap();
    assert_eq!(record.size, 9);
    assert_eq!(record.komi, 0.5);
    assert_eq!(record.setup_black, vec![at(9, 2, 2), at(9, 6, 6)]);
    assert!(record.setup_white.is_empty());

    // Only the main line, not the two variations that follow it.
    assert_eq!(
        record.moves,
        vec![
            (
                Color::White,
                Move::Play {
                    vertex: at(9, 4, 4)
                }
            ),
            (
                Color::Black,
                Move::Play {
                    vertex: at(9, 4, 6)
                }
            ),
        ]
    );
}

#[test]
fn handicap_setup_means_white_plays_first() {
    let record = sgf::parse("(;SZ[19]HA[2]AB[dd][pp];W[dp])").unwrap();
    let (game, failure) = sgf::replay(&record);
    assert_eq!(failure, None);
    assert_eq!(game.move_number(), 1);
    assert_eq!(
        game.to_play(),
        Color::Black,
        "white moved, so black is next"
    );
}

#[test]
fn round_trips_a_game_with_captures() {
    let mut original = Game::new(9, DEFAULT_KOMI);
    // Black surrounds and captures a white stone in the corner.
    for (color, row, col) in [
        (Color::Black, 0, 1),
        (Color::White, 0, 0),
        (Color::Black, 1, 0),
        (Color::White, 8, 8),
        (Color::Black, 1, 1),
        (Color::White, 8, 7),
    ] {
        original
            .play(
                color,
                Move::Play {
                    vertex: at(9, row, col),
                },
            )
            .expect("legal");
    }
    assert_eq!(original.captures(), [1, 0], "black took the corner stone");

    let text = sgf::to_sgf(&original);
    let record = sgf::parse(&text).unwrap();
    let (replayed, failure) = sgf::replay(&record);

    assert_eq!(failure, None, "the record we wrote must replay legally");
    assert_eq!(replayed.board(), original.board());
    assert_eq!(replayed.captures(), original.captures());
    assert_eq!(replayed.move_number(), original.move_number());
    assert_eq!(sgf::to_sgf(&replayed), text, "export is stable");
}

#[test]
fn ignores_comments_and_unknown_properties() {
    let text = "(;GM[1]SZ[19]C[a comment with a \\] bracket and ;B[zz] inside]\
                 ;B[pd]C[good move]TR[dd][pp];W[dp])";
    let record = sgf::parse(text).unwrap();
    assert_eq!(
        record.moves,
        vec![
            (
                Color::Black,
                Move::Play {
                    vertex: at(19, 3, 15)
                }
            ),
            (
                Color::White,
                Move::Play {
                    vertex: at(19, 15, 3)
                }
            ),
        ],
        "the escaped bracket and the text inside C[] must not become moves"
    );
}

#[test]
fn rejects_input_that_is_not_a_game_tree() {
    assert!(sgf::parse("not an sgf file").is_err());
    assert!(sgf::parse("(;SZ[99])").is_err(), "board size out of range");
}

/// A long generated game exercises the parser and the engine far harder than a
/// handful of hand-written positions: it accumulates captures, ko bans and
/// large groups, and every move must survive the export/parse/replay cycle.
#[test]
fn round_trips_a_long_generated_game() {
    let mut rng: u64 = 0x1234_5678_9abc_def0;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };

    let mut game = Game::new(19, DEFAULT_KOMI);
    let mut played = 0;
    let mut attempts = 0;

    while played < 300 && attempts < 20_000 {
        attempts += 1;
        let color = game.to_play();
        let vertex = (next() % 361) as u16;
        if game.play(color, Move::Play { vertex }).is_ok() {
            played += 1;
        }
    }
    assert_eq!(played, 300, "generated a full-length game");
    assert!(
        game.captures()[0] + game.captures()[1] > 0,
        "with captures in it"
    );

    let text = sgf::to_sgf(&game);
    let record = sgf::parse(&text).unwrap();
    let (replayed, failure) = sgf::replay(&record);

    assert_eq!(failure, None, "every move in our own export must replay");
    assert_eq!(replayed.board(), game.board());
    assert_eq!(replayed.captures(), game.captures());
}

#[test]
fn replay_reports_the_move_that_broke_the_rules() {
    // Two stones on the same point: the second must be reported, not silently
    // dropped. This is what makes replaying outside records a useful check.
    let record = SgfGame {
        size: 19,
        komi: DEFAULT_KOMI,
        setup_black: vec![],
        setup_white: vec![],
        moves: vec![
            (Color::Black, Move::Play { vertex: 0 }),
            (Color::White, Move::Play { vertex: 0 }),
        ],
    };
    let (_, failure) = sgf::replay(&record);
    assert_eq!(failure, Some((1, igo_core::MoveError::Occupied)));
}
