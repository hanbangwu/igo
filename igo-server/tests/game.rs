//! End-to-end tests over real WebSocket connections.

use serde_json::json;

mod common;
use common::*;

/// Seat both players and return once every client agrees they are seated.
/// Doubles as a barrier: moves sent afterwards cannot race the claims.
async fn seat_both(black: &mut Client, white: &mut Client) {
    black.send(&claim("black")).await;
    white.send(&claim("white")).await;
    black.expect_seats(Some("Black"), Some("White")).await;
    white.expect_seats(Some("Black"), Some("White")).await;
}

#[tokio::test]
async fn a_room_is_created_by_the_first_visitor() {
    let addr = spawn_default().await;
    let mut client = Client::join(addr, "abc123", "tok-a", "Ada").await;

    // Opening the same id reaches the same room, which is what makes sharing
    // a link work with no create-game endpoint.
    let mut other = Client::join(addr, "abc123", "tok-b", "Bo").await;
    other.send(&claim("black")).await;

    client.expect_seats(Some("Bo"), None).await;
}

#[tokio::test]
async fn two_players_claim_opposite_seats_and_play() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "game1", "tok-b", "Black").await;
    let mut white = Client::join(addr, "game1", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 3))).await;
    let applied = white.expect_move(0).await;
    assert_eq!(applied["color"], "black");
    assert_eq!(applied["move"]["vertex"], at(3, 3));
    assert_eq!(white.to_play(), "white");

    white.send(&play(1, at(15, 15))).await;
    let applied = black.expect_move(1).await;
    assert_eq!(applied["color"], "white");
    assert_eq!(black.to_play(), "black");
}

#[tokio::test]
async fn a_seat_cannot_be_taken_twice() {
    let addr = spawn_default().await;
    let mut first = Client::join(addr, "game2", "tok-1", "First").await;
    let mut second = Client::join(addr, "game2", "tok-2", "Second").await;

    first.send(&claim("black")).await;
    first.expect_seats(Some("First"), None).await;

    second.send(&claim("black")).await;
    let rejected = second.expect("rejected").await;
    assert!(
        rejected["reason"].as_str().unwrap().contains("taken"),
        "{rejected}"
    );
}

#[tokio::test]
async fn one_person_cannot_hold_both_seats() {
    let addr = spawn_default().await;
    let mut client = Client::join(addr, "game3", "same-token", "Solo").await;
    client.send(&claim("black")).await;
    client.expect_seats(Some("Solo"), None).await;

    // Opening a second tab does not make you a second person.
    let mut second_tab = Client::join(addr, "game3", "same-token", "Solo").await;
    second_tab.send(&claim("white")).await;
    let rejected = second_tab.expect("rejected").await;
    assert!(
        rejected["reason"]
            .as_str()
            .unwrap()
            .contains("other colour"),
        "{rejected}"
    );
}

#[tokio::test]
async fn spectators_cannot_move() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "game4", "tok-b", "Black").await;
    let mut white = Client::join(addr, "game4", "tok-w", "White").await;
    let mut watcher = Client::join(addr, "game4", "tok-s", "Watcher").await;
    seat_both(&mut black, &mut white).await;
    watcher.expect_seats(Some("Black"), Some("White")).await;

    watcher.send(&play(0, at(3, 3))).await;
    let rejected = watcher.expect("rejected").await;
    assert!(
        rejected["reason"].as_str().unwrap().contains("watching"),
        "{rejected}"
    );
}

#[tokio::test]
async fn moving_out_of_turn_is_refused() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "game5", "tok-b", "Black").await;
    let mut white = Client::join(addr, "game5", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    white.send(&play(0, at(3, 3))).await;
    let rejected = white.expect("rejected").await;
    assert!(
        rejected["reason"]
            .as_str()
            .unwrap()
            .contains("not your turn"),
        "{rejected}"
    );
}

#[tokio::test]
async fn a_stale_move_number_is_dropped() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "game6", "tok-b", "Black").await;
    let mut white = Client::join(addr, "game6", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 3))).await;
    black.expect_move(0).await;
    white.send(&play(1, at(15, 15))).await;
    black.expect_move(1).await;

    // Black composed this against the board before white replied.
    black.send(&play(1, at(9, 9))).await;
    let rejected = black.expect("rejected").await;
    assert!(
        rejected["reason"].as_str().unwrap().contains("stale"),
        "{rejected}"
    );
}

/// The behaviour deliberately changed from Rustpad, which kills the socket on
/// an invalid operation. Here a misclick must cost nothing.
#[tokio::test]
async fn an_illegal_move_is_refused_without_closing_the_socket() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "game7", "tok-b", "Black").await;
    let mut white = Client::join(addr, "game7", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 3))).await;
    white.expect_move(0).await;

    // White plays on top of black's stone.
    white.send(&play(1, at(3, 3))).await;
    let rejected = white.expect("rejected").await;
    assert!(
        rejected["reason"].as_str().unwrap().contains("occupied"),
        "{rejected}"
    );

    // The same connection then plays a legal move; nothing was torn down.
    white.send(&play(1, at(15, 15))).await;
    let applied = white.expect_move(1).await;
    assert_eq!(applied["color"], "white");
}

#[tokio::test]
async fn captures_are_reported_so_a_client_needs_no_rules() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "cap", "tok-b", "Black").await;
    let mut white = Client::join(addr, "cap", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(0, 1))).await;
    black.expect_move(0).await;
    white.send(&play(1, at(0, 0))).await;
    black.expect_move(1).await;

    // (0,0) now has one liberty; black takes it.
    black.send(&play(2, at(1, 0))).await;
    let applied = black.expect_move(2).await;
    assert_eq!(
        applied["captured"],
        json!([at(0, 0)]),
        "the client is told exactly which stones to lift: {applied}"
    );
    assert_eq!(black.captures(), &json!([1, 0]));
}

#[tokio::test]
async fn ko_is_refused_by_the_server() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "ko", "tok-b", "Black").await;
    let mut white = Client::join(addr, "ko", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    // Build the standard ko shape:
    //   . X O
    //   X . X O
    //   . X O
    let sequence = [
        at(1, 1),   // B
        at(1, 2),   // W
        at(2, 0),   // B
        at(2, 3),   // W
        at(2, 2),   // B
        at(3, 2),   // W
        at(3, 1),   // B
        at(10, 10), // W, elsewhere
        at(12, 12), // B, elsewhere, so white is on move for the capture
    ];
    let mut n = 0;
    for vertex in sequence {
        let client = if n % 2 == 0 { &mut black } else { &mut white };
        client.send(&play(n, vertex)).await;
        black.expect_move(n).await;
        white.expect_move(n).await;
        n += 1;
    }

    white.send(&play(n, at(2, 1))).await;
    let applied = black.expect_move(n).await;
    assert_eq!(
        applied["captured"],
        json!([at(2, 2)]),
        "white takes the ko: {applied}"
    );
    n += 1;

    black.send(&play(n, at(2, 2))).await;
    let rejected = black.expect("rejected").await;
    assert!(
        rejected["reason"].as_str().unwrap().contains("ko"),
        "{rejected}"
    );
}

#[tokio::test]
async fn reconnecting_with_the_same_token_restores_the_seat() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "resume", "tok-b", "Black").await;
    let mut white = Client::join(addr, "resume", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 3))).await;
    black.expect_move(0).await;

    // Black refreshes the page.
    drop(black);
    let mut reconnected = Client::connect(addr, "resume").await;
    reconnected
        .send(&json!({ "type": "hello", "token": "tok-b", "name": "Black" }))
        .await;

    let identity = reconnected.expect("identity").await;
    assert_eq!(identity["you"], "black", "the seat survived the disconnect");

    let snapshot = reconnected.expect("snapshot").await;
    assert_eq!(snapshot["you"], "black");
    assert_eq!(snapshot["move_number"], 1);
    assert_eq!(snapshot["to_play"], "white");
    assert_eq!(snapshot["seats"]["black"], "Black");
    assert_eq!(
        snapshot["board"][at(3, 3) as usize],
        1,
        "the board came back with the stone on it"
    );
}

#[tokio::test]
async fn a_different_token_does_not_inherit_a_seat() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "steal", "tok-b", "Black").await;
    black.send(&claim("black")).await;
    black.expect_seats(Some("Black"), None).await;
    drop(black);

    let mut impostor = Client::join(addr, "steal", "tok-x", "Impostor").await;
    impostor.send(&play(0, at(3, 3))).await;
    let rejected = impostor.expect("rejected").await;
    assert!(
        rejected["reason"].as_str().unwrap().contains("watching"),
        "{rejected}"
    );
}

#[tokio::test]
async fn two_passes_enter_scoring_and_agreement_ends_the_game() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "score", "tok-b", "Black").await;
    let mut white = Client::join(addr, "score", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 3))).await;
    white.expect_move(0).await;
    white.send(&play(1, at(15, 15))).await;
    white.expect_move(1).await;

    black.send(&pass(2)).await;
    white.expect_move(2).await;
    white.send(&pass(3)).await;

    white.expect_move(3).await;
    assert_eq!(white.phase()["state"], "scoring");

    // Marks start empty and both players must agree.
    let dead = white.expect("dead").await;
    assert_eq!(dead["vertices"], json!([]));
    assert_eq!(dead["accepted"], json!([]));

    black.send(&json!({ "type": "accept_score" })).await;
    let dead = black
        .expect_matching("dead", |v| v["accepted"] == json!(["black"]))
        .await;
    assert_eq!(dead["accepted"], json!(["black"]), "one is not enough");

    white.send(&json!({ "type": "accept_score" })).await;
    let phase = white.expect("phase_changed").await;
    assert_eq!(phase["phase"]["state"], "finished");
    assert_eq!(
        phase["phase"]["result"]["kind"], "counted",
        "counted out rather than resigned: {phase}"
    );
}

#[tokio::test]
async fn marking_stones_dead_clears_previous_agreement() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "marks", "tok-b", "Black").await;
    let mut white = Client::join(addr, "marks", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 3))).await;
    black.expect_move(0).await;
    white.send(&pass(1)).await;
    black.expect_move(1).await;
    black.send(&pass(2)).await;
    black.expect_move(2).await;

    black.send(&json!({ "type": "accept_score" })).await;
    black
        .expect_matching("dead", |v| v["accepted"] == json!(["black"]))
        .await;

    // White changes the marks, so black's agreement no longer applies.
    white
        .send(&json!({ "type": "toggle_dead", "vertex": at(3, 3) }))
        .await;
    let dead = white
        .expect_matching("dead", |v| v["vertices"] == json!([at(3, 3)]))
        .await;
    assert_eq!(
        dead["accepted"],
        json!([]),
        "black must look at the new marks before agreeing again"
    );
}

#[tokio::test]
async fn players_can_resume_play_when_they_disagree() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "resume2", "tok-b", "Black").await;
    let mut white = Client::join(addr, "resume2", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&pass(0)).await;
    white.expect_move(0).await;
    white.send(&pass(1)).await;
    white.expect_move(1).await;

    black.send(&json!({ "type": "resume_play" })).await;
    let phase = white.expect("phase_changed").await;
    assert_eq!(phase["phase"]["state"], "playing");

    // Play continues from where it left off.
    black.send(&play(2, at(3, 3))).await;
    let applied = black.expect_move(2).await;
    assert_eq!(applied["move"]["vertex"], at(3, 3));
    assert_eq!(black.move_count(), 3);
}

#[tokio::test]
async fn a_spectator_cannot_mark_stones() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "watch", "tok-b", "Black").await;
    let mut white = Client::join(addr, "watch", "tok-w", "White").await;
    let mut watcher = Client::join(addr, "watch", "tok-s", "Watcher").await;
    seat_both(&mut black, &mut white).await;
    watcher.expect_seats(Some("Black"), Some("White")).await;

    black.send(&play(0, at(3, 3))).await;
    watcher.expect_move(0).await;
    white.send(&pass(1)).await;
    watcher.expect_move(1).await;
    black.send(&pass(2)).await;
    watcher.expect("dead").await;

    watcher
        .send(&json!({ "type": "toggle_dead", "vertex": at(3, 3) }))
        .await;
    let rejected = watcher.expect("rejected").await;
    assert!(
        rejected["reason"]
            .as_str()
            .unwrap()
            .contains("only the players"),
        "{rejected}"
    );
}

#[tokio::test]
async fn resignation_ends_the_game_immediately() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "resign", "tok-b", "Black").await;
    let mut white = Client::join(addr, "resign", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black
        .send(&json!({
            "type": "play",
            "move_number": 0,
            "move": { "type": "resign" },
        }))
        .await;

    white.expect_move(0).await;
    assert_eq!(white.phase()["state"], "finished");
    assert_eq!(white.phase()["result"]["kind"], "resignation");
    assert_eq!(white.phase()["result"]["winner"], "white");
}

#[tokio::test]
async fn a_seat_can_be_released_only_before_the_first_move() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "release", "tok-b", "Black").await;
    let mut white = Client::join(addr, "release", "tok-w", "White").await;

    black.send(&claim("black")).await;
    black.expect_seats(Some("Black"), None).await;
    black.send(&json!({ "type": "release_seat" })).await;
    black.expect_seats(None, None).await;

    // Reclaim, start the game, then try again.
    seat_both(&mut black, &mut white).await;
    black.send(&play(0, at(3, 3))).await;
    black.expect_move(0).await;

    black.send(&json!({ "type": "release_seat" })).await;
    let rejected = black.expect("rejected").await;
    assert!(
        rejected["reason"]
            .as_str()
            .unwrap()
            .contains("already started"),
        "{rejected}"
    );
}

#[tokio::test]
async fn an_unrecognised_message_closes_the_connection() {
    let addr = spawn_default().await;
    let mut client = Client::join(addr, "bad", "tok-b", "Black").await;
    client.send(&json!({ "type": "no_such_message" })).await;
    client.expect_closed().await;
}

#[tokio::test]
async fn a_connection_must_identify_itself_first() {
    let addr = spawn_default().await;
    let mut client = Client::connect(addr, "nohello").await;
    client.send(&claim("black")).await;
    client.expect_closed().await;
}

#[tokio::test]
async fn sgf_is_downloadable_and_replays() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "sgfroom", "tok-b", "Black").await;
    let mut white = Client::join(addr, "sgfroom", "tok-w", "White").await;
    seat_both(&mut black, &mut white).await;

    black.send(&play(0, at(3, 15))).await;
    black.expect_move(0).await;
    white.send(&play(1, at(15, 3))).await;
    black.expect_move(1).await;

    let body = http_body(&format!("http://{addr}/api/sgf/sgfroom")).await;
    assert!(body.contains(";B[pd];W[dp]"), "{body}");

    let record = igo_core::sgf::parse(&body).expect("our own export parses");
    let (game, failure) = igo_core::sgf::replay(&record);
    assert_eq!(failure, None);
    assert_eq!(game.move_number(), 2);
}

#[tokio::test]
async fn unknown_rooms_and_bad_ids_are_rejected() {
    let addr = spawn_default().await;
    assert_eq!(
        http_status(&format!("http://{addr}/api/sgf/never-played")).await,
        404
    );
    assert_eq!(
        http_status(&format!("http://{addr}/api/sgf/has%20spaces")).await,
        400
    );
    assert_eq!(
        http_status(&format!("http://{addr}/api/sgf/{}", "x".repeat(64))).await,
        400
    );
}

// A hand-rolled GET, so the test suite does not pull in an HTTP client for
// three calls.
async fn http_get(url: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let rest = url.strip_prefix("http://").expect("http url");
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut stream = tokio::net::TcpStream::connect(authority).await.unwrap();
    let request = format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

async fn http_body(url: &str) -> String {
    http_get(url)
        .await
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default()
}

async fn http_status(url: &str) -> u16 {
    http_get(url)
        .await
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

/// Seats are broadcast to everyone, but *which* seat you hold is private to
/// your connection. Without a fresh identity the player who just sat down
/// would never learn they had, and the client would keep the board read-only.
#[tokio::test]
async fn claiming_a_seat_tells_that_connection_who_it_is() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "identity", "tok-b", "Black").await;
    let mut white = Client::join(addr, "identity", "tok-w", "White").await;

    black.send(&claim("black")).await;
    let identity = black
        .expect_matching("identity", |v| v["you"] == "black")
        .await;
    assert_eq!(identity["you"], "black");

    white.send(&claim("white")).await;
    white
        .expect_matching("identity", |v| v["you"] == "white")
        .await;

    // Leaving the seat has to clear it again, or the client would keep
    // offering moves it is no longer allowed to make.
    black.send(&json!({ "type": "release_seat" })).await;
    let identity = black
        .expect_matching("identity", |v| v["you"].is_null())
        .await;
    assert!(identity["you"].is_null(), "{identity}");
}

/// A second tab of the same player must also learn it holds the seat.
#[tokio::test]
async fn another_tab_of_the_same_player_learns_its_seat() {
    let addr = spawn_default().await;
    let mut first = Client::join(addr, "tabs", "tok-b", "Black").await;
    let mut second = Client::join(addr, "tabs", "tok-b", "Black").await;

    first.send(&claim("black")).await;
    second
        .expect_matching("identity", |v| v["you"] == "black")
        .await;
}

/// The client renames by sending another hello, so a repeat must be a rename
/// rather than an error — otherwise changing your name flashes a refusal.
#[tokio::test]
async fn a_second_hello_renames_the_player() {
    let addr = spawn_default().await;
    let mut black = Client::join(addr, "rename", "tok-b", "Black").await;
    black.send(&claim("black")).await;
    black.expect_seats(Some("Black"), None).await;

    black
        .send(&json!({ "type": "hello", "token": "tok-b", "name": "Renamed" }))
        .await;
    black.expect_seats(Some("Renamed"), None).await;

    // Swapping token on a live connection would silently change who you are.
    black
        .send(&json!({ "type": "hello", "token": "someone-else", "name": "Nope" }))
        .await;
    let rejected = black.expect("rejected").await;
    assert!(
        rejected["reason"]
            .as_str()
            .unwrap()
            .contains("change identity"),
        "{rejected}"
    );
}

/// An empty or absurd name must not reach the other player's screen as-is.
#[tokio::test]
async fn names_are_trimmed_and_defaulted() {
    let addr = spawn_default().await;
    let mut client = Client::join(addr, "names", "tok-b", "   ").await;
    client.send(&claim("black")).await;
    client.expect_seats(Some("Anonymous"), None).await;

    let long = "x".repeat(200);
    client
        .send(&json!({ "type": "hello", "token": "tok-b", "name": long }))
        .await;
    let seats = client
        .expect_matching("seats", |v| v["seats"]["black"] != "Anonymous")
        .await;
    assert_eq!(
        seats["seats"]["black"].as_str().unwrap().chars().count(),
        40,
        "clamped to a sane length"
    );
}
