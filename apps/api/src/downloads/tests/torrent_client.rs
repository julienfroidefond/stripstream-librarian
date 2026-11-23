//! The qBittorrent port: the poll verdict as a table, and the adapter's wire
//! format against a mock server.

use std::time::Duration;

use chrono::{DateTime, Utc};
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

// ─── States ─────────────────────────────────────────────────────────────────

#[test]
fn qbittorrent_states_are_classified() {
    for state in [
        "uploading",
        "stalledUP",
        "pausedUP",
        "stoppedUP",
        "queuedUP",
    ] {
        assert_eq!(TorrentState::of(state), TorrentState::Completed, "{state}");
    }
    for state in ["pausedDL", "stoppedDL"] {
        assert_eq!(TorrentState::of(state), TorrentState::Paused, "{state}");
    }
    assert_eq!(TorrentState::of("stalledDL"), TorrentState::Stalled);
    for state in ["error", "missingFiles"] {
        assert_eq!(TorrentState::of(state), TorrentState::Failed, "{state}");
    }
    for state in [
        "downloading",
        "metaDL",
        "queuedDL",
        "checkingDL",
        "forcedDL",
        "moving",
    ] {
        assert_eq!(TorrentState::of(state), TorrentState::InProgress, "{state}");
    }
}

#[test]
fn a_stall_expires_only_after_the_timeout() {
    let now = Utc::now();
    let timeout = Duration::from_secs(30 * 60);
    assert!(!stall_expired(now, now, timeout));
    assert!(!stall_expired(
        now - chrono::Duration::minutes(29),
        now,
        timeout
    ));
    assert!(stall_expired(
        now - chrono::Duration::minutes(30),
        now,
        timeout
    ));
}

#[test]
fn missing_hashes_ignore_case() {
    let tracked = vec!["AABB".to_string(), "ccdd".to_string(), "eeff".to_string()];
    assert_eq!(missing_hashes(&tracked, &["aabb", "CCDD"]), vec!["eeff"]);
    assert!(missing_hashes(&tracked, &["aabb", "ccdd", "eeff"]).is_empty());
}

// ─── The decision table ─────────────────────────────────────────────────────

fn tracked(hash: &str, stall_started_at: Option<DateTime<Utc>>) -> TrackedTorrent {
    TrackedTorrent {
        id: Uuid::new_v4(),
        hash: hash.to_string(),
        created_at: Utc::now(),
        qb_owned: true,
        stall_started_at,
    }
}

#[test]
fn the_poll_verdict_follows_the_state_and_the_stall_marker() {
    let now = Utc::now();
    let timeout = Duration::from_secs(30 * 60);
    let verdict = |state: &str, stall: Option<DateTime<Utc>>| {
        decide(
            &TorrentInfo::test("ab", state),
            &tracked("ab", stall),
            now,
            timeout,
        )
    };

    for state in [
        "uploading",
        "stalledUP",
        "pausedUP",
        "stoppedUP",
        "queuedUP",
    ] {
        assert_eq!(verdict(state, None), Decision::Complete, "{state}");
    }

    for state in ["pausedDL", "stoppedDL"] {
        assert_eq!(verdict(state, None), Decision::LeaveAlone, "{state}");
    }

    for state in ["error", "missingFiles"] {
        match verdict(state, Some(now - chrono::Duration::minutes(90))) {
            Decision::Fail(message) => assert!(message.contains(state), "{message}"),
            other => panic!("{state}: {other:?}"),
        }
    }

    // A stall is only a failure once its marker outlived the timeout.
    assert_eq!(verdict("stalledDL", None), Decision::Stall, "never armed");
    assert_eq!(
        verdict("stalledDL", Some(now - chrono::Duration::minutes(29))),
        Decision::Stall
    );
    match verdict("stalledDL", Some(now - chrono::Duration::minutes(30))) {
        Decision::Fail(message) => {
            assert!(
                message.contains("stalled in qBittorrent for 30 min"),
                "{message}"
            );
            assert!(message.contains("stalledDL"), "{message}");
        }
        other => panic!("{other:?}"),
    }

    for state in [
        "downloading",
        "metaDL",
        "queuedDL",
        "checkingDL",
        "forcedDL",
        "moving",
    ] {
        assert_eq!(verdict(state, None), Decision::Track, "{state}");
    }
}

#[test]
fn a_completed_torrent_is_complete_by_label_or_by_fraction() {
    assert!(TorrentInfo::test("ab", "uploading")
        .with_progress(0.4)
        .is_completed());
    assert!(TorrentInfo::test("ab", "downloading")
        .with_progress(1.0)
        .is_completed());
    assert!(!TorrentInfo::test("ab", "downloading")
        .with_progress(0.99)
        .is_completed());
}

// ─── Adapter ────────────────────────────────────────────────────────────────

fn client(server: &MockServer) -> QbittorrentClient {
    QbittorrentClient::from_parts(server.uri(), reqwest::Client::new(), "abc".to_string())
}

#[tokio::test]
async fn torrents_by_hash_normalizes_the_wire_entry() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .and(query_param("hashes", "aa|bb"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "hash": "aa",
                "state": "downloading",
                "content_path": "/downloads/sl-1/Amulet T01.cbz",
                "save_path": "/downloads/sl-1",
                "name": "Amulet T01",
                "category": "sl-1",
                "progress": 0.42,
                "total_size": 1024,
                "dlspeed": 128,
                "eta": 60,
            }])),
        )
        .mount(&server)
        .await;

    let infos = client(&server)
        .torrents_by_hash(&["aa".to_string(), "bb".to_string()])
        .await
        .unwrap();

    assert_eq!(infos.len(), 1);
    let info = &infos[0];
    assert_eq!(info.hash, "aa");
    assert_eq!(info.state, "downloading");
    assert_eq!(info.state_kind(), TorrentState::InProgress);
    assert_eq!(
        info.content_path.as_deref(),
        Some("/downloads/sl-1/Amulet T01.cbz")
    );
    assert_eq!(info.save_path.as_deref(), Some("/downloads/sl-1"));
    assert_eq!(info.name.as_deref(), Some("Amulet T01"));
    assert_eq!(info.category.as_deref(), Some("sl-1"));
    assert_eq!(info.progress, 0.42);
    assert_eq!(info.total_size, 1024);
    assert_eq!(info.speed, 128, "dlspeed is normalized to speed");
    assert_eq!(info.eta, 60);
}

#[tokio::test]
async fn an_unusable_info_answer_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client(&server)
        .torrents_by_hash(&["aa".to_string()])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("500"), "{err}");
}

#[tokio::test]
async fn delete_sends_the_file_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/delete"))
        .and(body_string_contains("hashes=ab"))
        .and(body_string_contains("deleteFiles=true"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client(&server).delete("ab", true).await.unwrap();
}

#[tokio::test]
async fn a_refused_delete_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/delete"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client(&server).delete("ab", false).await.unwrap_err();
    assert!(
        err.to_string().contains("refused to remove torrent ab"),
        "{err}"
    );
}

#[tokio::test]
async fn resolve_hash_uses_the_single_torrent_of_the_category() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .and(query_param("category", "sl-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "hash": "ab01", "state": "downloading", "name": "Amulet T01",
            }])),
        )
        .mount(&server)
        .await;

    assert_eq!(
        client(&server).resolve_hash_by_category("sl-1").await,
        Some("ab01".to_string())
    );
}

#[tokio::test]
async fn an_ambiguous_category_resolves_to_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .and(query_param("category", "sl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "hash": "ab01", "state": "downloading" },
            { "hash": "ab02", "state": "downloading" },
        ])))
        .mount(&server)
        .await;

    assert_eq!(client(&server).resolve_hash_by_category("sl-1").await, None);
}

#[tokio::test]
async fn an_uncategorized_torrent_is_found_by_its_directory_and_filed() {
    let server = MockServer::start().await;
    // The torrent qBittorrent filed nowhere, but stored in the `sl-1` directory.
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "hash": "ab01",
                "state": "downloading",
                "save_path": "/downloads/sl-1",
                "category": "",
            }])),
        )
        .mount(&server)
        .await;
    // The category query itself comes back empty: only the fallback can match.
    Mock::given(method("GET"))
        .and(path("/api/v2/torrents/info"))
        .and(query_param("category", "sl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/torrents/setCategory"))
        .and(body_string_contains("hashes=ab01"))
        .and(body_string_contains("category=sl-1"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    assert_eq!(
        client(&server).resolve_hash_by_category("sl-1").await,
        Some("ab01".to_string())
    );
}
