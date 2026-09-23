use std::time::Duration;

use igo_server::ServerConfig;
use serde_json::json;

mod common;
use common::*;

async fn spawn_impatient() -> std::net::SocketAddr {
    spawn(ServerConfig {
        max_idle: Duration::from_millis(50),
        sweep_interval: Duration::from_millis(25),
        static_dir: "/nonexistent".into(),
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn idle_rooms_are_reclaimed() {
    let addr = spawn_impatient().await;

    let client = Client::join(addr, "ephemeral", "tok-a", "Ada").await;
    assert_eq!(room_count(addr).await, 1);
    drop(client);

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(room_count(addr).await, 0, "the room should have been swept");
}

#[tokio::test]
async fn reclaiming_a_room_drops_its_connections() {
    let addr = spawn_impatient().await;
    let mut client = Client::join(addr, "doomed", "tok-a", "Ada").await;

    client.expect_closed().await;
}

#[tokio::test]
async fn a_game_in_progress_is_not_swept_while_it_is_being_used() {
    let addr = spawn(ServerConfig {
        max_idle: Duration::from_millis(400),
        sweep_interval: Duration::from_millis(25),
        static_dir: "/nonexistent".into(),
        ..Default::default()
    })
    .await;

    let mut black = Client::join(addr, "busy", "tok-b", "Black").await;
    let mut white = Client::join(addr, "busy", "tok-w", "White").await;
    black.send(&claim("black")).await;
    white.send(&claim("white")).await;
    black.expect_seats(Some("Black"), Some("White")).await;

    for n in 0..4 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let mut toucher = Client::join(addr, "busy", "tok-s", "Watcher").await;
        toucher.expect_seats(Some("Black"), Some("White")).await;
        assert_eq!(room_count(addr).await, 1, "still alive after touch {n}");
    }

    black.send(&play(0, at(3, 3))).await;
    let applied = white.expect_move(0).await;
    assert_eq!(applied["move"]["vertex"], at(3, 3));
}

#[tokio::test]
async fn stats_reports_live_rooms() {
    let addr = spawn_default().await;
    assert_eq!(room_count(addr).await, 0);

    let _a = Client::join(addr, "room-a", "tok-a", "Ada").await;
    let _b = Client::join(addr, "room-b", "tok-b", "Bo").await;
    let _c = Client::join(addr, "room-b", "tok-c", "Cy").await;

    assert_eq!(room_count(addr).await, 2);
}

async fn room_count(addr: std::net::SocketAddr) -> u64 {
    let body = http_body(&format!("http://{addr}/api/stats")).await;
    let stats: serde_json::Value = serde_json::from_str(&body).unwrap_or(json!({}));
    stats["rooms"].as_u64().unwrap_or(u64::MAX)
}

async fn http_body(url: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let rest = url.strip_prefix("http://").expect("http url");
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut stream = tokio::net::TcpStream::connect(authority).await.unwrap();
    let request = format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default()
}
