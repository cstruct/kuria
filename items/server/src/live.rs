//! Editing live drafts together (`live.sync`, `live.instances`,
//! `live.presence`): `GET /akter/{number}/live`, a WebSocket speaking the
//! y-websocket sync and awareness protocols.
//!
//! - [`room`]: the clients of one live draft on this instance, what they
//!   are sent, and who they are.
//! - [`socket`]: one client's connection.
//! - [`presence`]: awareness, stamped with who each client is.
//!
//! An update a client sends is taken through the write path
//! ([`Akter::take_live_update`]), kept with who sent it, and reaches every
//! client from Postgres: its room reads the draft's updates after the last
//! one it sent, when woken and every second, so that updates written by
//! anything else (`editAkt`, a push) reach them too. Between instances, a
//! Valkey channel per akt says only that the draft has new updates, which
//! each instance then reads from Postgres, and carries awareness, which
//! isn't kept.

mod presence;
mod room;
mod socket;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::StreamExt;
use konklav_axum::{Policy, RoutePolicyError, RouterPolicy, TenantToken};
use redis::aio::ConnectionManager;
use tokio::sync::broadcast;
use uuid::Uuid;
use yrs::sync::{AwarenessUpdate, Message};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use crate::akter::live::Opened;
use crate::akter::{AktError, Akter, Caller};
use crate::authorize::Decider;
use room::{Outgoing, Room};

/// The Valkey channels of live drafts: one per akt.
const CHANNELS: &str = "kuria:live:*";
/// What a message on a channel says: that the draft has new updates.
const UPDATED: u8 = 0;
/// What a message on a channel says: awareness, as an encoded update.
const AWARENESS: u8 = 1;

/// The Valkey channel of the live draft of `akt`, in `tenant`.
fn channel(tenant: Uuid, akt: Uuid) -> String {
    format!("kuria:live:{tenant}:{akt}")
}

/// This instance's rooms, one per live draft some client of it has open.
#[derive(Clone)]
pub struct Rooms {
    inner: Arc<Inner>,
}

struct Inner {
    akter: Akter,
    valkey: Option<redis::Client>,
    publisher: tokio::sync::Mutex<Option<ConnectionManager>>,
    rooms: Mutex<HashMap<Uuid, Arc<Room>>>,
    /// Who this instance is on the channels, to skip what it published.
    instance: Uuid,
    listening: std::sync::atomic::AtomicBool,
    next_client: std::sync::atomic::AtomicU64,
    /// The cursors shown without a connection (`live.presence.shown`), by
    /// draft and awareness client: the clock each was last shown at.
    shown: Mutex<HashMap<(Uuid, u64), u32>>,
}

impl Rooms {
    /// Rooms writing through `akter`, reaching other instances through the
    /// Valkey server `valkey` names, if any.
    #[must_use]
    pub fn new(akter: Akter, valkey: Option<redis::Client>) -> Self {
        Self {
            inner: Arc::new(Inner {
                akter,
                valkey,
                publisher: tokio::sync::Mutex::new(None),
                rooms: Mutex::new(HashMap::new()),
                instance: Uuid::now_v7(),
                listening: std::sync::atomic::AtomicBool::new(false),
                next_client: std::sync::atomic::AtomicU64::new(1),
                shown: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn akter(&self) -> &Akter {
        &self.inner.akter
    }

    fn rooms(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, Arc<Room>>> {
        self.inner
            .rooms
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Joins the room of the live draft `opened`, starting it if none is.
    async fn join(
        &self,
        tenant: Uuid,
        opened: &Opened,
    ) -> Result<(Arc<Room>, broadcast::Receiver<Arc<Outgoing>>), AktError> {
        if let Some(room) = self.rooms().get(&opened.akt) {
            return Ok((room.clone(), room.subscribe()));
        }
        // What the room has sent starts at what a client reads after
        // joining, so that nothing falls between.
        let mut conn = self.akter().pool().get().await?;
        let last_seq = kuria_store::drafts::last_seq(&mut conn, tenant, opened.akt).await?;
        drop(conn);
        self.listen();
        let mut rooms = self.rooms();
        if let Some(room) = rooms.get(&opened.akt) {
            return Ok((room.clone(), room.subscribe()));
        }
        let room = Arc::new(Room::new(
            tenant,
            opened.akt,
            opened.number.clone(),
            last_seq,
        ));
        let receiver = room.subscribe();
        rooms.insert(opened.akt, room.clone());
        drop(rooms);
        tokio::spawn(room.clone().run(self.clone()));
        Ok((room, receiver))
    }

    /// Forgets `room` if no client is in it any more, and says whether it
    /// did.
    fn close_if_empty(&self, room: &Room) -> bool {
        let mut rooms = self.rooms();
        if room.is_empty() {
            rooms.remove(&room.akt);
            true
        } else {
            false
        }
    }

    /// Tells the other instances that `room`'s draft has new updates
    /// (`live.instances`).
    async fn published_update(&self, room: &Room) {
        self.publish(room, UPDATED, &[]).await;
    }

    /// Tells the other instances `room`'s awareness `update`
    /// (`live.instances`).
    async fn published_awareness(&self, room: &Room, update: &AwarenessUpdate) {
        self.publish(room, AWARENESS, &update.encode_v1()).await;
    }

    // r[impl live.instances]
    async fn publish(&self, room: &Room, tag: u8, body: &[u8]) {
        self.publish_on(room.tenant, room.akt, tag, body).await;
    }

    async fn publish_on(&self, tenant: Uuid, akt: Uuid, tag: u8, body: &[u8]) {
        let Some(client) = &self.inner.valkey else {
            return;
        };
        let mut publisher = self.inner.publisher.lock().await;
        if publisher.is_none() {
            match client.get_connection_manager().await {
                Ok(conn) => *publisher = Some(conn),
                Err(e) => {
                    tracing::warn!("reaching Valkey for live drafts: {e}");
                    return;
                }
            }
        }
        let Some(conn) = publisher.as_mut() else {
            return;
        };
        let mut message = Vec::with_capacity(17 + body.len());
        message.push(tag);
        message.extend_from_slice(self.inner.instance.as_bytes());
        message.extend_from_slice(body);
        let sent: redis::RedisResult<()> = redis::cmd("PUBLISH")
            .arg(channel(tenant, akt))
            .arg(message)
            .query_async(conn)
            .await;
        if let Err(e) = sent {
            tracing::warn!("publishing to a live draft's channel: {e}");
        }
    }

    /// Shows the caller's cursor at `span` of `field` in the live draft of
    /// the akt `number`, without a connection of theirs, and says they left
    /// [`SHOWN_FOR`] after unless shown again (`live.presence.shown`).
    // r[impl live.presence.shown]
    pub async fn show_cursor(
        &self,
        caller: Caller<'_>,
        number: &str,
        field: &str,
        span: &str,
    ) -> Result<(), AktError> {
        let opened = self.akter().open_live(caller, number).await?;
        let tenant: Uuid = caller.actor.tenant.into();
        let draft = self.akter().live_draft(tenant, number).await?;
        let cursor = draft.cursor(field, span).map_err(AktError::Refused)?;
        let (subject, acting) = caller.actor.by();
        let name = presence::name_of(self.akter(), tenant, subject, acting).await?;
        // One awareness client per person, and agent, in each draft, so
        // that showing again moves their cursor.
        let who = {
            use sha2::Digest as _;
            let mut hash = sha2::Sha256::new();
            hash.update(opened.akt.as_bytes());
            hash.update(subject.as_bytes());
            hash.update(acting.unwrap_or_default().as_bytes());
            hash.finalize()
        };
        let client =
            u64::from_le_bytes(who[..8].try_into().unwrap_or_default()) & 0x001f_ffff_ffff_ffff;
        let clock = shown_clock();
        let state = serde_json::json!({ "cursor": cursor }).to_string();
        let update = presence::stamp(awareness(client, clock, &state), subject, &name);
        self.inner
            .shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert((opened.akt, client), clock);
        self.present(tenant, opened.akt, &update).await;
        let rooms = self.clone();
        let akt = opened.akt;
        tokio::spawn(async move {
            tokio::time::sleep(SHOWN_FOR).await;
            let still = {
                let mut shown = rooms
                    .inner
                    .shown
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let still = shown.get(&(akt, client)) == Some(&clock);
                if still {
                    shown.remove(&(akt, client));
                }
                still
            };
            if still {
                rooms
                    .present(tenant, akt, &awareness(client, clock + 1, "null"))
                    .await;
            }
        });
        Ok(())
    }

    /// Sends `update`, awareness from no connection, to the draft's clients
    /// here and on the other instances (`live.instances`).
    async fn present(&self, tenant: Uuid, akt: Uuid, update: &AwarenessUpdate) {
        let room = self.rooms().get(&akt).cloned();
        if let Some(room) = room {
            let taken = room.take_presence(None, update.clone());
            if !taken.clients.is_empty() {
                room.broadcast(None, Message::Awareness(taken).encode_v1());
            }
        }
        self.publish_on(tenant, akt, AWARENESS, &update.encode_v1())
            .await;
    }

    /// Listens to the other instances' channels, once.
    fn listen(&self) {
        use std::sync::atomic::Ordering;
        let Some(client) = self.inner.valkey.clone() else {
            return;
        };
        if self.inner.listening.swap(true, Ordering::SeqCst) {
            return;
        }
        let rooms = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = rooms.listen_once(&client).await {
                    tracing::warn!("listening to live drafts' channels: {e}");
                }
                // Whatever was missed meanwhile is read from Postgres.
                for room in rooms.rooms().values() {
                    room.wake();
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    // r[impl live.instances]
    async fn listen_once(&self, client: &redis::Client) -> redis::RedisResult<()> {
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.psubscribe(CHANNELS).await?;
        let mut messages = pubsub.on_message();
        while let Some(message) = messages.next().await {
            let channel = message.get_channel_name();
            let Some(akt) = channel
                .rsplit(':')
                .next()
                .and_then(|id| Uuid::parse_str(id).ok())
            else {
                continue;
            };
            let payload = message.get_payload_bytes();
            let Some((&tag, rest)) = payload.split_first() else {
                continue;
            };
            let Some((from, body)) = rest.split_at_checked(16) else {
                continue;
            };
            if from == self.inner.instance.as_bytes() {
                continue;
            }
            let Some(room) = self.rooms().get(&akt).cloned() else {
                continue;
            };
            match tag {
                UPDATED => room.wake(),
                AWARENESS => {
                    if let Ok(update) = AwarenessUpdate::decode_v1(body) {
                        room.remote_presence(update);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// How long a cursor shown without a connection stays
/// (`live.presence.shown`).
const SHOWN_FOR: Duration = Duration::from_secs(10);

/// An awareness clock that grows with time, a tenth of a second at a time,
/// so that a cursor shown again, by any instance, is newer.
fn shown_clock() -> u32 {
    let since = chrono::Utc::now().timestamp_millis() - 1_767_225_600_000;
    u32::try_from(since.max(0) / 100).unwrap_or(u32::MAX - 1)
}

/// An awareness update of `client` at `clock`, saying `state`.
fn awareness(client: u64, clock: u32, state: &str) -> AwarenessUpdate {
    AwarenessUpdate {
        clients: HashMap::from([(
            yrs::block::ClientID::new(client),
            yrs::sync::awareness::AwarenessUpdateEntry {
                clock,
                json: Arc::from(state),
            },
        )]),
    }
}

/// What the live route needs.
#[derive(Clone)]
struct LiveState {
    rooms: Rooms,
    decider: Arc<dyn Decider>,
}

/// `GET /akter/{number}/live`, authenticated as `/graphql` is, through
/// `api`.
// r[impl live.sync]
// r[impl live.instances]
pub(crate) fn router(
    api: &RouterPolicy,
    rooms: Rooms,
    decider: Arc<dyn Decider>,
) -> Result<Router, RoutePolicyError> {
    Ok(Router::new()
        .route(
            "/akter/{number}/live",
            get(route).route_layer(api.route(&Policy::new())?),
        )
        .with_state(LiveState { rooms, decider }))
}

/// Upgrades to a WebSocket of the live draft of the akt `number`, as the
/// subject and in the tenant of the request's verified token; an akt
/// without one, or that the subject may not read, is not found.
// r[impl live.sync]
async fn route(
    State(state): State<LiveState>,
    Path(number): Path<String>,
    token: TenantToken,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Edits are decided long after the upgrade request is answered.
    let (principal, auth) = match crate::http::caller(&token, &headers, &state.decider, None) {
        Ok(caller) => caller,
        Err(refused) => return *refused,
    };
    let rooms = state.rooms;
    let caller = Caller {
        actor: principal.actor(),
        auth: &auth,
        storage: None,
        renderer: None,
    };
    let unavailable = |e: AktError| {
        tracing::warn!("opening a live draft: {e:?}");
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    };
    let opened = match rooms.akter().open_live(caller, &number).await {
        Ok(opened) => opened,
        Err(AktError::NotFound(_)) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return unavailable(e),
    };
    let tenant: Uuid = principal.tenant.into();
    let name = match presence::name_of(
        rooms.akter(),
        tenant,
        principal.subject.into(),
        principal.acting.map(Into::into),
    )
    .await
    {
        Ok(name) => name,
        Err(e) => return unavailable(e),
    };
    let (room, receiver) = match rooms.join(tenant, &opened).await {
        Ok(joined) => joined,
        Err(e) => return unavailable(e),
    };
    let client = socket::Client {
        id: rooms
            .inner
            .next_client
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        rooms: rooms.clone(),
        room,
        receiver,
        principal,
        auth,
        may_edit: opened.may_edit,
        name,
    };
    upgrade.on_upgrade(move |socket| client.serve(socket))
}
