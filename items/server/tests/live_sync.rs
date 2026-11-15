//! Editing a live draft together over `GET /akter/{number}/live`, with
//! Yjs clients written with `yrs`, against two server instances on local
//! ports sharing a fresh database and the development Valkey
//! (`live.sync`, `live.instances`, `live.presence`).

mod support;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use futures_util::{SinkExt, StreamExt};
use kuria_server::authorize::READ;
use kuria_server::changes::{Hub, Relay};
use kuria_server::{Changes, Options};
use kuria_store::schema::change_event;
use kuria_test_support::{TestDatabase, valkey_url};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use yrs::sync::awareness::AwarenessUpdateEntry;
use yrs::sync::{AwarenessUpdate, Message, MessageReader, SyncMessage};
use yrs::types::ToJson;
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::Encode;
use yrs::{Doc, GetString, Map, ReadTxn, StateVector, Text, Transact, Update};

use support::decider::FakeDecider;
use support::{COLLEAGUE, FakeHost, PUBLIC_URL, SUBJECT, graphql_with, send};

/// One server instance on a local port, with its own relay and hub.
struct Instance {
    router: axum::Router,
    address: std::net::SocketAddr,
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
        Self { router, address }
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with("tenant", query, &variables))
            .await
            .json();
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    /// Connects to the live draft of `number` as `token`.
    async fn connect(&self, token: &str, number: &str) -> Result<Client, u16> {
        let mut request = format!("ws://{}/akter/{number}/live", self.address)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        let socket = match tokio_tungstenite::connect_async(request).await {
            Ok((socket, _)) => socket,
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                return Err(response.status().as_u16());
            }
            Err(e) => panic!("connecting: {e}"),
        };
        let mut client = Client {
            socket,
            doc: Doc::new(),
            refusals: Vec::new(),
            presence: HashMap::new(),
            synced: false,
        };
        client
            .send(&Message::Sync(SyncMessage::SyncStep1(
                StateVector::default(),
            )))
            .await;
        client.until(|c| c.synced).await;
        Ok(client)
    }
}

/// A y-websocket client, its Yjs document kept with `yrs`.
struct Client {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    doc: Doc,
    /// Why the server refused what the client sent, as it said.
    refusals: Vec<String>,
    /// The awareness states it was sent, by client.
    presence: HashMap<u64, Value>,
    /// Whether it had the server's sync step 2.
    synced: bool,
}

impl Client {
    async fn send(&mut self, message: &Message) {
        self.socket
            .send(Frame::binary(message.encode_v1()))
            .await
            .unwrap();
    }

    /// Reads and handles messages until `done` holds, for up to five
    /// seconds.
    async fn until(&mut self, done: impl Fn(&Self) -> bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !done(self) {
            let frame = tokio::time::timeout_at(deadline, self.socket.next())
                .await
                .expect("the condition within five seconds")
                .expect("an open socket")
                .unwrap();
            let Frame::Binary(bytes) = frame else {
                continue;
            };
            let mut decoder = DecoderV1::from(bytes.as_ref());
            let messages: Vec<Message> = MessageReader::new(&mut decoder)
                .map(Result::unwrap)
                .collect();
            for message in messages {
                self.handle(message).await;
            }
        }
    }

    async fn handle(&mut self, message: Message) {
        match message {
            Message::Sync(SyncMessage::SyncStep1(sv)) => {
                let missing = self.doc.transact().encode_state_as_update_v1(&sv);
                self.send(&Message::Sync(SyncMessage::SyncStep2(missing)))
                    .await;
            }
            Message::Sync(SyncMessage::SyncStep2(update)) => {
                self.apply(&update);
                self.synced = true;
            }
            Message::Sync(SyncMessage::Update(update)) => self.apply(&update),
            Message::Auth(Some(reason)) => self.refusals.push(reason),
            Message::Awareness(update) => {
                for (id, entry) in update.clients {
                    self.presence
                        .insert(id.get(), serde_json::from_str(&entry.json).unwrap());
                }
            }
            _ => {}
        }
    }

    fn apply(&self, update: &[u8]) {
        self.doc
            .transact_mut()
            .apply_update(Update::decode_v1(update).unwrap())
            .unwrap();
    }

    fn text(&self, key: &str) -> String {
        let text = self.doc.get_or_insert_text(key);
        text.get_string(&self.doc.transact())
    }

    fn value(&self, key: &str) -> Option<Value> {
        let map = self.doc.get_or_insert_map("fields");
        let txn = self.doc.transact();
        map.get(&txn, key)
            .map(|out| serde_json::to_value(out.to_json(&txn)).unwrap())
    }

    /// Makes an edit with `edit`, given the draft's text `key` and its
    /// map, and sends it.
    async fn edit(
        &mut self,
        key: &str,
        edit: impl FnOnce(&yrs::TextRef, &yrs::MapRef, &mut yrs::TransactionMut),
    ) {
        // The roots first: getting one waits for any open transaction.
        let text = self.doc.get_or_insert_text(key);
        let map = self.doc.get_or_insert_map("fields");
        let update = {
            let mut txn = self.doc.transact_mut();
            edit(&text, &map, &mut txn);
            txn.encode_update_v1()
        };
        self.send(&Message::Sync(SyncMessage::Update(update))).await;
    }

    /// Inserts `text` at `at` in the text `key`, and sends it.
    async fn insert(&mut self, key: &str, at: u32, text: &str) {
        self.edit(key, |root, _, txn| root.insert(txn, at, text))
            .await;
    }
}

fn document() -> Value {
    json!({
        "key": "document", "name": "Document", "number": "D-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown" }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "versions": {},
        "live_drafts": true
    })
}

struct World {
    db: TestDatabase,
    first: Instance,
    second: Instance,
    number: String,
}

impl World {
    /// Two instances, a live document `D-1` titled "Guide" with the body
    /// "Hello.", Ada (`tenant`) and Grace (`colleague`) who may edit it,
    /// and Grace as a reader (`colleague-reader`).
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider = FakeDecider::allowing_tenant();
        decider.allow_everything("colleague");
        decider.grant("colleague-reader", &[READ]);
        let first = Instance::start(&db, decider.clone()).await;
        let second = Instance::start(&db, decider).await;
        let mut conn = db.pool().get().await.unwrap();
        for (subject, name) in [(SUBJECT, "Ada"), (COLLEAGUE, "Grace")] {
            kuria_store::people::record(
                &mut conn,
                &kuria_store::people::Person {
                    subject_id: subject.parse().unwrap(),
                    name: Some(name.to_owned()),
                    email: None,
                    updated_at: chrono::Utc::now(),
                },
            )
            .await
            .unwrap();
        }
        drop(conn);
        first
            .ok(
                "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                json!({ "d": document() }),
            )
            .await;
        let number = first
            .ok(
                "mutation { createAkt(input: { type: \"document\", fields: { title: \"Guide\", body: \"Hello.\" } }) { number } }",
                json!({}),
            )
            .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned();
        Self {
            db,
            first,
            second,
            number,
        }
    }

    /// The akt's fields and the authors of its draft's changes, as GraphQL
    /// answers them.
    async fn read(&self) -> (Value, Vec<String>) {
        let akt = self
            .first
            .ok(
                "query($n: String!) { akt(number: $n) {
                    fields { ... on TextValue { field text } ... on MarkdownValue { field markdown } }
                    draftChanges(all: true) { by { id } fields } } }",
                json!({ "n": self.number }),
            )
            .await["akt"]
            .clone();
        let by = akt["draftChanges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["by"]["id"].as_str().unwrap().to_owned())
            .collect();
        (akt["fields"].clone(), by)
    }
}

// r[verify live.sync]
// r[verify live.instances]
// r[verify live.history]
#[tokio::test(flavor = "multi_thread")]
async fn two_clients_on_two_instances_edit_one_draft_together() {
    let world = World::new().await;
    let mut ada = world.first.connect("tenant", &world.number).await.unwrap();
    let mut grace = world
        .second
        .connect("colleague", &world.number)
        .await
        .unwrap();
    // Both have the draft.
    for client in [&ada, &grace] {
        assert_eq!(client.text("body"), "Hello.");
        assert_eq!(client.value("title"), Some(json!("Guide")));
    }

    ada.insert("body", 6, " Welcome.").await;
    grace.until(|c| c.text("body") == "Hello. Welcome.").await;
    grace.insert("body", 0, "# Intro\n\n").await;
    ada.until(|c| c.text("body") == "# Intro\n\nHello. Welcome.")
        .await;

    // Kept, with who made each change, as the akt's fields.
    let (fields, by) = world.read().await;
    assert_eq!(
        fields,
        json!([
            { "field": "title", "text": "Guide" },
            { "field": "body", "markdown": "# Intro\n\nHello. Welcome." }
        ])
    );
    assert_eq!(by, [SUBJECT, COLLEAGUE]);

    // A client connecting later has it all.
    let late = world.second.connect("tenant", &world.number).await.unwrap();
    assert_eq!(late.text("body"), "# Intro\n\nHello. Welcome.");

    // The edits are change events, so lists refresh.
    let mut conn = world.db.pool().get().await.unwrap();
    let mut drafts = 0;
    for _ in 0..50 {
        drafts = change_event::table
            .filter(change_event::kind.eq("draft"))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .unwrap();
        if drafts > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(drafts > 0);
}

// r[verify live.sync]
#[tokio::test(flavor = "multi_thread")]
async fn an_edit_made_elsewhere_reaches_every_client() {
    let world = World::new().await;
    let mut ada = world.first.connect("tenant", &world.number).await.unwrap();
    world
        .second
        .ok(
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 1, fields: { body: \"Hello, world.\" } }) { revision } }",
            json!({ "n": world.number }),
        )
        .await;
    ada.until(|c| c.text("body") == "Hello, world.").await;
}

// r[verify live.sync]
#[tokio::test(flavor = "multi_thread")]
async fn a_readers_update_is_not_taken() {
    let world = World::new().await;
    let mut ada = world.first.connect("tenant", &world.number).await.unwrap();
    let mut reader = world
        .first
        .connect("colleague-reader", &world.number)
        .await
        .unwrap();
    assert_eq!(reader.text("body"), "Hello.");
    // Told at once, and again when it edits.
    let read_only = "you may read this live draft, not edit it";
    assert_eq!(reader.refusals, [read_only]);
    reader.insert("body", 0, "Not mine. ").await;
    reader.until(|c| c.refusals.len() == 2).await;
    assert_eq!(reader.refusals, [read_only, read_only]);
    // A reader still gets every change.
    ada.insert("body", 6, " Welcome.").await;
    reader.until(|c| c.text("body").ends_with("Welcome.")).await;
    assert_eq!(world.read().await.1, [SUBJECT]);
    assert_eq!(ada.text("body"), "Hello. Welcome.");
}

// r[verify live.sync]
// r[verify live.edit]
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_update_is_undone_for_every_client() {
    let world = World::new().await;
    let mut ada = world.first.connect("tenant", &world.number).await.unwrap();
    let mut grace = world
        .second
        .connect("colleague", &world.number)
        .await
        .unwrap();
    // The title is required.
    ada.edit("body", |_, map, txn| {
        map.remove(txn, "title");
    })
    .await;
    ada.until(|c| !c.refusals.is_empty()).await;
    assert_eq!(ada.refusals, ["`title` is required"]);
    ada.until(|c| c.value("title") == Some(json!("Guide")))
        .await;
    // Grace never loses it, and both go on as one.
    grace.insert("body", 6, " Hi.").await;
    ada.until(|c| c.text("body") == "Hello. Hi.").await;
    assert_eq!(grace.value("title"), Some(json!("Guide")));
    let (fields, _) = world.read().await;
    assert_eq!(fields[0], json!({ "field": "title", "text": "Guide" }));
}

// r[verify live.presence]
// r[verify live.instances]
#[tokio::test(flavor = "multi_thread")]
async fn awareness_says_who_each_client_is() {
    let world = World::new().await;
    let mut ada = world.first.connect("tenant", &world.number).await.unwrap();
    let mut grace = world
        .second
        .connect("colleague", &world.number)
        .await
        .unwrap();
    let id = ada.doc.client_id();
    ada.send(&Message::Awareness(AwarenessUpdate {
        clients: HashMap::from([(
            id,
            AwarenessUpdateEntry {
                clock: 1,
                json: Arc::from(r#"{"user":{"name":"Mallory"},"cursor":{"anchor":2,"head":4}}"#),
            },
        )]),
    }))
    .await;
    grace.until(|c| c.presence.contains_key(&id.get())).await;
    let state = &grace.presence[&id.get()];
    assert_eq!(state["user"], json!({ "id": SUBJECT, "name": "Ada" }));
    assert_eq!(state["cursor"], json!({ "anchor": 2, "head": 4 }));
    // Someone joining later is told who is there.
    let late = world
        .first
        .connect("colleague", &world.number)
        .await
        .unwrap();
    let mut late = late;
    late.until(|c| c.presence.contains_key(&id.get())).await;
    // Leaving says so.
    drop(ada);
    grace
        .until(|c| c.presence.get(&id.get()) == Some(&Value::Null))
        .await;
}

// r[verify live.sync]
#[tokio::test(flavor = "multi_thread")]
async fn an_akt_without_a_live_draft_is_not_found() {
    let world = World::new().await;
    world
        .first
        .ok(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": {
                "key": "item", "name": "Item", "number": "I-{seq}",
                "fields": [{ "key": "title", "name": "Title", "kind": "text" }],
                "states": [{ "key": "open", "name": "Open", "initial": true }]
            } }),
        )
        .await;
    let item = world
        .first
        .ok(
            "mutation { createAkt(input: { type: \"item\" }) { number } }",
            json!({}),
        )
        .await["createAkt"]["number"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(world.first.connect("tenant", &item).await.err(), Some(404));
    assert_eq!(world.first.connect("tenant", "D-99").await.err(), Some(404));
    // Nor is one the subject may not read.
    assert_eq!(
        world
            .first
            .connect("tenant-nobody", &world.number)
            .await
            .err(),
        Some(404)
    );
}
