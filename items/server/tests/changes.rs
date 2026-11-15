//! Change events from the outbox, through Valkey, to `changes` subscriptions
//! over WebSocket, against a real server on a local port, a fresh database
//! and the development Valkey.

mod support;

use std::sync::Arc;
use std::time::Duration;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use futures_util::{SinkExt, StreamExt};
use kuria_server::authorize::READ;
use kuria_server::changes::{Hub, Relay, stream_key};
use kuria_server::{Changes, Options};
use kuria_store::schema::change_event;
use kuria_test_support::{TestDatabase, valkey_url};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use support::decider::FakeDecider;
use support::{FakeHost, PUBLIC_URL, TENANT, graphql_with, send};

const ALICE: &str = "tenant-alice";

/// One server instance: its router, served on a local port, with its own
/// relay and hub.
struct Instance {
    router: axum::Router,
    address: std::net::SocketAddr,
    relay: Relay,
}

impl Instance {
    async fn start(db: &TestDatabase, decider: Arc<FakeDecider>) -> Self {
        let client = redis::Client::open(valkey_url()).unwrap();
        let valkey = client.get_connection_manager().await.unwrap();
        let relay = Relay::new(db.pool().clone(), valkey);
        let hub = Hub::new(db.pool().clone(), client);
        tokio::spawn(relay.clone().run());
        tokio::spawn(hub.clone().run());
        let router = kuria_server::app(
            FakeHost::new(),
            Options {
                public_url: PUBLIC_URL.to_owned(),
                trusted_proxies: Vec::new(),
                pool: db.pool().clone(),
                decider,
                sessions: None,
                changes: Some(Changes {
                    hub,
                    waker: relay.waker(),
                }),
                storage: kuria_server::attachments::Storage::in_memory(1 << 20),
                web: None,
                login_proxy: None,
                cli_relay: None,
                renderer: support::renderer(),
                git_cache: None,
                git_timing: kuria_server::GitTiming::default(),
            },
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let served = router.clone();
        tokio::spawn(async move {
            axum::serve(
                listener,
                served.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });
        Self {
            router,
            address,
            relay,
        }
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with("tenant", query, &variables))
            .await
            .json();
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str) -> String {
        self.ok(
            "mutation($t: String!) { createAkt(input: { type: $t }) { number } }",
            json!({ "t": key }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Opens a `changes` subscription as `token`, after `after` if given.
    async fn subscribe(&self, token: &str, after: Option<i64>) -> Subscriber {
        let mut request = format!("ws://{}/graphql/ws", self.address)
            .into_client_request()
            .unwrap();
        let headers = request.headers_mut();
        headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        headers.insert(
            "sec-websocket-protocol",
            "graphql-transport-ws".parse().unwrap(),
        );
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        socket
            .send(Message::text(
                json!({ "type": "connection_init" }).to_string(),
            ))
            .await
            .unwrap();
        let ack = next_message(&mut socket).await.unwrap();
        assert_eq!(ack["type"], "connection_ack", "{ack}");
        socket
            .send(Message::text(
                json!({
                    "id": "1",
                    "type": "subscribe",
                    "payload": {
                        "query": "subscription($after: Int) { changes(after: $after) { seq kind number revision } }",
                        "variables": { "after": after },
                    },
                })
                .to_string(),
            ))
            .await
            .unwrap();
        Subscriber { socket }
    }
}

struct Subscriber {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Subscriber {
    /// The next change event, or `None` if the subscription completed.
    async fn next(&mut self) -> Option<Value> {
        loop {
            let message = next_message(&mut self.socket).await?;
            match message["type"].as_str() {
                Some("next") => return Some(message["payload"]["data"]["changes"].clone()),
                Some("complete") => return None,
                Some("error") => panic!("subscription failed: {message}"),
                _ => {}
            }
        }
    }

    /// The next `n` change events as `(seq, number)` pairs.
    async fn take(&mut self, n: usize) -> Vec<(i64, String)> {
        let mut events = Vec::new();
        for _ in 0..n {
            let event = self.next().await.expect("an event");
            events.push((
                event["seq"].as_i64().unwrap(),
                event["number"].as_str().unwrap().to_owned(),
            ));
        }
        events
    }
}

/// The next text message, waiting up to five seconds; `None` if the socket
/// closed.
async fn next_message(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Option<Value> {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("a message within five seconds")?
            .ok()?;
        match message {
            Message::Text(text) => return Some(serde_json::from_str(&text).unwrap()),
            Message::Close(_) => return None,
            _ => {}
        }
    }
}

async fn define(instance: &Instance) {
    for (key, prefix) in [("item", "ITEM"), ("secret", "SECRET")] {
        instance
            .ok(
                "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                json!({ "d": {
                    "key": key, "name": key, "number": format!("{prefix}-{{seq}}"),
                    "fields": [{ "key": "title", "name": "Title", "kind": "text" }],
                    "states": [{ "key": "open", "name": "Open", "initial": true }]
                } }),
            )
            .await;
    }
}

// r[verify changes.subscribe]
// r[verify changes.websocket]
#[tokio::test(flavor = "multi_thread")]
async fn delivers_each_change_in_order() {
    let db = TestDatabase::new().await;
    let instance = Instance::start(&db, FakeDecider::allowing_tenant()).await;
    define(&instance).await;
    // From the start of the tenant's sequence, so nothing depends on how
    // soon the subscription joins its hub.
    let mut subscriber = instance.subscribe("tenant", Some(0)).await;

    let first = instance.create("item").await;
    let second = instance.create("item").await;
    instance
        .ok(
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 1, fields: { title: \"T\" } }) { revision } }",
            json!({ "n": first }),
        )
        .await;
    instance
        .ok(
            "mutation($n: String!, $o: String!) { moveAkt(input: { number: $n, before: $o }) { number } }",
            json!({ "n": second, "o": first }),
        )
        .await;

    let mut received = Vec::new();
    for _ in 0..4 {
        received.push(subscriber.next().await.expect("an event"));
    }
    let seqs: Vec<i64> = received
        .iter()
        .map(|e| e["seq"].as_i64().unwrap())
        .collect();
    assert_eq!(seqs, [1, 2, 3, 4]);
    assert_eq!(
        received[0],
        json!({ "seq": 1, "kind": "REVISION", "number": first, "revision": 1 })
    );
    assert_eq!(received[2]["revision"], 2);
    assert_eq!(
        received[3],
        json!({ "seq": 4, "kind": "RANK", "number": second, "revision": null })
    );
}

// r[verify changes.catch-up]
#[tokio::test(flavor = "multi_thread")]
async fn catches_up_from_a_cursor_then_continues() {
    let db = TestDatabase::new().await;
    let instance = Instance::start(&db, FakeDecider::allowing_tenant()).await;
    define(&instance).await;
    instance.create("item").await;
    let second = instance.create("item").await;
    let third = instance.create("item").await;

    let mut subscriber = instance.subscribe("tenant", Some(1)).await;
    assert_eq!(subscriber.take(2).await, [(2, second), (3, third)]);
    let fourth = instance.create("item").await;
    assert_eq!(subscriber.take(1).await, [(4, fourth)]);

    // Without a cursor, only what happens from now on.
    let mut fresh = instance.subscribe("tenant", None).await;
    // Without a cursor the subscription only sees what happens after it
    // joined, so give it time to join.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let fifth = instance.create("item").await;
    assert_eq!(fresh.take(1).await, [(5, fifth)]);
}

// r[verify changes.visibility]
#[tokio::test(flavor = "multi_thread")]
async fn leaves_out_what_the_subscriber_cant_read() {
    let db = TestDatabase::new().await;
    let decider = FakeDecider::allowing_tenant();
    decider.grant_at(ALICE, "item", &[READ]);
    let instance = Instance::start(&db, decider).await;
    define(&instance).await;
    instance.create("secret").await;
    let item = instance.create("item").await;
    instance.create("secret").await;
    let later = instance.create("item").await;
    let mut subscriber = instance.subscribe(ALICE, Some(0)).await;
    assert_eq!(subscriber.take(2).await, [(2, item), (4, later)]);
}

// r[verify changes.publish]
// r[verify changes.identifiers]
#[tokio::test(flavor = "multi_thread")]
async fn publishes_every_event_to_its_tenants_stream() {
    let db = TestDatabase::new().await;
    let instance = Instance::start(&db, FakeDecider::allowing_tenant()).await;
    define(&instance).await;
    instance.create("item").await;
    // Whatever the background relay hasn't done, a pass now finishes.
    instance.relay.publish_pending().await.unwrap();
    let mut conn = db.pool().get().await.unwrap();
    let unpublished: i64 = change_event::table
        .filter(change_event::published_at.is_null())
        .count()
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(unpublished, 0);

    let mut valkey = redis::Client::open(valkey_url())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let entries: Vec<(String, Vec<(String, String)>)> = redis::cmd("XREVRANGE")
        .arg(stream_key(TENANT.parse().unwrap()))
        .arg("+")
        .arg("-")
        .arg("COUNT")
        .arg(1)
        .query_async(&mut valkey)
        .await
        .unwrap();
    let (_, fields) = &entries[0];
    assert_eq!(fields.len(), 1, "only the sequence number: {fields:?}");
    assert_eq!(fields[0].0, "seq");
}

// r[verify changes.publish]
#[tokio::test(flavor = "multi_thread")]
async fn a_change_on_one_instance_reaches_subscribers_of_another() {
    let db = TestDatabase::new().await;
    let writer = Instance::start(&db, FakeDecider::allowing_tenant()).await;
    let reader = Instance::start(&db, FakeDecider::allowing_tenant()).await;
    define(&writer).await;
    let mut subscriber = reader.subscribe("tenant", Some(0)).await;
    // The first event comes from catching up; the second must travel
    // through Valkey from the writer to the reader's hub.
    let first = writer.create("item").await;
    assert_eq!(subscriber.take(1).await, [(1, first)]);
    let item = writer.create("item").await;
    assert_eq!(subscriber.take(1).await, [(2, item)]);
}

// r[verify changes.expiry]
#[tokio::test(flavor = "multi_thread")]
async fn ends_when_the_token_expires() {
    let db = TestDatabase::new().await;
    let instance = Instance::start(&db, FakeDecider::allowing_tenant()).await;
    define(&instance).await;
    let mut subscriber = instance.subscribe("tenant-expiring", None).await;
    let started = std::time::Instant::now();
    assert_eq!(subscriber.next().await, None, "completed, not an event");
    assert!(started.elapsed() < Duration::from_secs(4));
}
