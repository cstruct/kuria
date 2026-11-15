//! The clients of one live draft on this instance: what they are all sent,
//! and who is there.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{Notify, broadcast};
use uuid::Uuid;
use yrs::ClientID;
use yrs::sync::awareness::AwarenessUpdateEntry;
use yrs::sync::{AwarenessUpdate, Message, SyncMessage};
use yrs::updates::encoder::Encode;

use super::Rooms;

/// How many messages a client may fall behind before it is sent the whole
/// draft instead.
const BACKLOG: usize = 256;
/// How often a room reads its draft's updates from Postgres without being
/// woken: what an edit made other than through a room waits at most.
const POLL: Duration = Duration::from_secs(1);
/// How often at most a room records that its draft changed, as a change
/// event (`live.sync`).
const DEBOUNCE: Duration = Duration::from_secs(1);
/// How long awareness from another instance lasts without being renewed,
/// as y-protocols' clients renew theirs every 15 seconds.
const OUTDATED: Duration = Duration::from_secs(30);

/// A message for the clients of a room.
pub(super) struct Outgoing {
    /// The client it came from, which isn't sent it back.
    pub from: Option<u64>,
    /// The encoded y-websocket message.
    pub message: Vec<u8>,
}

/// One awareness client's state, as the room knows it.
struct Peer {
    clock: u32,
    /// Its state, as JSON; `null` once it left.
    json: Arc<str>,
    /// The client of this instance it belongs to; `None` for one of
    /// another instance's.
    client: Option<u64>,
    seen: Instant,
}

#[derive(Default)]
struct State {
    /// The draft's last update sent to the room's clients.
    last_seq: i64,
    peers: HashMap<ClientID, Peer>,
    /// Whether a client's edit hasn't been recorded as a change event yet.
    edited: bool,
}

/// The clients of one live draft on this instance.
pub(super) struct Room {
    pub tenant: Uuid,
    pub akt: Uuid,
    pub number: String,
    sender: broadcast::Sender<Arc<Outgoing>>,
    wake: Notify,
    state: Mutex<State>,
}

impl Room {
    pub fn new(tenant: Uuid, akt: Uuid, number: String, last_seq: i64) -> Self {
        Self {
            tenant,
            akt,
            number,
            sender: broadcast::channel(BACKLOG).0,
            wake: Notify::new(),
            state: Mutex::new(State {
                last_seq,
                ..State::default()
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Outgoing>> {
        self.sender.subscribe()
    }

    pub fn is_empty(&self) -> bool {
        self.sender.receiver_count() == 0
    }

    /// Sends `message` to every client of the room but `from`.
    pub fn broadcast(&self, from: Option<u64>, message: Vec<u8>) {
        // No receivers is fine: they may all have just left.
        let _ = self.sender.send(Arc::new(Outgoing { from, message }));
    }

    /// Has the room read its draft's new updates now.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// A client's edit was taken: the room sends it, and records it as a
    /// change event soon.
    pub fn edited(&self) {
        self.state().edited = true;
        self.wake();
    }

    /// Takes the awareness `update` from the room's client `client`, or
    /// from another instance's if `None`, and answers what of it is new:
    /// a client's entries for awareness clients another one claimed are
    /// left out (`live.presence`).
    // r[impl live.presence]
    pub fn take_presence(&self, client: Option<u64>, update: AwarenessUpdate) -> AwarenessUpdate {
        let mut state = self.state();
        let now = Instant::now();
        let mut taken = HashMap::new();
        for (id, entry) in update.clients {
            let gone = &*entry.json == "null";
            match state.peers.get(&id) {
                Some(peer)
                    if client.is_some() && peer.client.is_some() && peer.client != client =>
                {
                    continue;
                }
                Some(peer) if peer.clock > entry.clock || (peer.clock == entry.clock && !gone) => {
                    continue;
                }
                _ => {}
            }
            state.peers.insert(
                id,
                Peer {
                    clock: entry.clock,
                    json: entry.json.clone(),
                    client: if gone { None } else { client },
                    seen: now,
                },
            );
            taken.insert(id, entry);
        }
        AwarenessUpdate { clients: taken }
    }

    /// Takes another instance's awareness `update` and sends what is new to
    /// the room's clients.
    pub fn remote_presence(&self, update: AwarenessUpdate) {
        let taken = self.take_presence(None, update);
        if !taken.clients.is_empty() {
            self.broadcast(None, Message::Awareness(taken).encode_v1());
        }
    }

    /// Everyone in the room now, and in the other instances' rooms of its
    /// draft, as one awareness update; `None` if no one is.
    pub fn presence(&self) -> Option<AwarenessUpdate> {
        let state = self.state();
        let clients: HashMap<ClientID, AwarenessUpdateEntry> = state
            .peers
            .iter()
            .filter(|(_, peer)| {
                &*peer.json != "null" && (peer.client.is_some() || peer.seen.elapsed() < OUTDATED)
            })
            .map(|(id, peer)| {
                (
                    *id,
                    AwarenessUpdateEntry {
                        clock: peer.clock,
                        json: peer.json.clone(),
                    },
                )
            })
            .collect();
        (!clients.is_empty()).then_some(AwarenessUpdate { clients })
    }

    /// The room's client `client` left: the awareness update that says
    /// its awareness clients left too, `None` if it had none.
    pub fn leave(&self, client: u64) -> Option<AwarenessUpdate> {
        let mut state = self.state();
        let mut gone = HashMap::new();
        for (id, peer) in &mut state.peers {
            if peer.client == Some(client) {
                peer.clock += 1;
                peer.json = Arc::from("null");
                peer.client = None;
                gone.insert(
                    *id,
                    AwarenessUpdateEntry {
                        clock: peer.clock,
                        json: peer.json.clone(),
                    },
                );
            }
        }
        (!gone.is_empty()).then_some(AwarenessUpdate { clients: gone })
    }

    /// Sends the room's clients its draft's updates as they are kept, and
    /// records its clients' edits as change events, until no client is
    /// left.
    // r[impl live.sync]
    // r[impl live.instances]
    pub async fn run(self: Arc<Self>, rooms: Rooms) {
        let mut recorded: Option<Instant> = None;
        loop {
            let wait = match recorded {
                Some(at) if self.state().edited => DEBOUNCE.saturating_sub(at.elapsed()).min(POLL),
                _ => POLL,
            };
            tokio::select! {
                () = self.wake.notified() => {}
                () = tokio::time::sleep(wait) => {}
            }
            if rooms.close_if_empty(&self) {
                // What a client edited is still recorded.
                if self.state().edited {
                    self.record(&rooms).await;
                }
                return;
            }
            self.send_updates(&rooms).await;
            if self.state().edited && recorded.is_none_or(|at| at.elapsed() >= DEBOUNCE) {
                self.record(&rooms).await;
                recorded = Some(Instant::now());
            }
        }
    }

    /// Sends the room's clients the draft's updates after the last one it
    /// sent, from Postgres.
    async fn send_updates(&self, rooms: &Rooms) {
        let after = self.state().last_seq;
        let Ok(mut conn) = rooms.akter().pool().get().await else {
            tracing::warn!("checking out a connection for a live draft");
            return;
        };
        let updates =
            match kuria_store::drafts::updates_after(&mut conn, self.tenant, self.akt, after).await
            {
                Ok(updates) => updates,
                Err(e) => {
                    tracing::warn!("reading a live draft's updates: {e}");
                    return;
                }
            };
        drop(conn);
        for update in updates {
            {
                let mut state = self.state();
                // Another pass may have sent it meanwhile.
                if update.seq <= state.last_seq {
                    continue;
                }
                state.last_seq = update.seq;
            }
            self.broadcast(
                None,
                Message::Sync(SyncMessage::Update(update.update)).encode_v1(),
            );
        }
    }

    /// Records that the room's draft changed, as a change event.
    async fn record(&self, rooms: &Rooms) {
        self.state().edited = false;
        if let Err(e) = rooms
            .akter()
            .live_draft_changed(self.tenant, &self.number)
            .await
        {
            tracing::warn!("recording a live draft's change: {e:?}");
        }
    }
}
