//! One client's connection to a live draft: the y-websocket sync and
//! awareness protocols (`live.sync`, `live.presence`).
//!
//! On connecting, the server sends its sync step 1 and who else is there;
//! the client answers with what the server lacks and sends its own step 1,
//! which the server answers with what the client lacks. From then on each
//! side sends its updates. A refused update is answered with an auth
//! message saying why, and undone by the update every client is then sent.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message as Frame, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::{self, error::RecvError};
use uuid::Uuid;
use yrs::sync::{Message, MessageReader, SyncMessage};
use yrs::updates::decoder::DecoderV1;
use yrs::updates::encoder::Encode;

use super::room::{Outgoing, Room};
use super::{Rooms, presence};
use crate::akter::live::Taken;
use crate::akter::{AktError, Caller};
use crate::authorize::Authorizer;
use crate::graphql::Principal;

/// What a reader is told on connecting, and of an update they sent.
const READ_ONLY: &str = "you may read this live draft, not edit it";

/// One client of a room.
pub(super) struct Client {
    pub id: u64,
    pub rooms: Rooms,
    pub room: Arc<Room>,
    pub receiver: broadcast::Receiver<Arc<Outgoing>>,
    pub principal: Principal,
    pub auth: Arc<Authorizer>,
    pub may_edit: bool,
    pub name: String,
}

/// What handling a client's message leads to.
enum Handled {
    /// These replies, to the client alone.
    Reply(Vec<Vec<u8>>),
    /// The connection can't go on: the client reconnects and syncs anew.
    End(Option<String>),
}

impl Client {
    /// Serves the client until it leaves, or its token expires.
    // r[impl live.sync]
    pub async fn serve(mut self, socket: WebSocket) {
        let (mut sink, mut stream) = socket.split();
        let remaining = self.principal.expires_at - chrono::Utc::now().timestamp();
        let deadline = tokio::time::Instant::now()
            + Duration::from_secs(u64::try_from(remaining).unwrap_or(0));
        let mut opening = Vec::new();
        match self.draft().await {
            Ok(draft) => opening.push(sync_step_1(&draft.state_vector())),
            Err(e) => {
                tracing::warn!("loading a live draft: {e:?}");
                return;
            }
        }
        if let Some(present) = self.room.presence() {
            opening.push(Message::Awareness(present).encode_v1());
        }
        // A reader is told at once that their edits won't be taken.
        if !self.may_edit {
            opening.push(denied(READ_ONLY));
        }
        for message in opening {
            if sink.send(Frame::Binary(message.into())).await.is_err() {
                return self.leave().await;
            }
        }
        loop {
            let replies = tokio::select! {
                () = tokio::time::sleep_until(deadline) => break,
                frame = stream.next() => match frame {
                    Some(Ok(Frame::Binary(bytes))) => self.handle(&bytes).await,
                    Some(Ok(Frame::Close(_)) | Err(_)) | None => break,
                    Some(Ok(_)) => continue,
                },
                outgoing = self.receiver.recv() => match outgoing {
                    Ok(outgoing) if outgoing.from == Some(self.id) => continue,
                    Ok(outgoing) => Handled::Reply(vec![outgoing.message.clone()]),
                    // Fallen behind: the whole draft brings it level.
                    Err(RecvError::Lagged(_)) => match self.draft().await {
                        Ok(draft) => Handled::Reply(vec![
                            Message::Sync(SyncMessage::Update(draft.state())).encode_v1(),
                        ]),
                        Err(_) => Handled::End(None),
                    },
                    Err(RecvError::Closed) => break,
                },
            };
            match replies {
                Handled::Reply(messages) => {
                    for message in messages {
                        if sink.send(Frame::Binary(message.into())).await.is_err() {
                            return self.leave().await;
                        }
                    }
                }
                Handled::End(reason) => {
                    if let Some(reason) = reason {
                        let _ = sink
                            .send(Frame::Binary(
                                Message::Auth(Some(reason)).encode_v1().into(),
                            ))
                            .await;
                    }
                    break;
                }
            }
        }
        let _ = sink.close().await;
        self.leave().await;
    }

    async fn draft(&self) -> Result<kuria_engine::live::LiveDraft, AktError> {
        self.rooms
            .akter()
            .live_draft(self.room.tenant, &self.room.number)
            .await
    }

    /// Handles a frame, which may hold several messages.
    async fn handle(&mut self, bytes: &[u8]) -> Handled {
        let mut decoder = DecoderV1::from(bytes);
        let messages: Vec<_> = MessageReader::new(&mut decoder).collect();
        let mut replies = Vec::new();
        for message in messages {
            let Ok(message) = message else {
                return Handled::End(Some("a message that isn't y-websocket's".to_owned()));
            };
            match self.handle_one(message).await {
                Handled::Reply(more) => replies.extend(more),
                end @ Handled::End(_) => return end,
            }
        }
        Handled::Reply(replies)
    }

    async fn handle_one(&mut self, message: Message) -> Handled {
        match message {
            Message::Sync(SyncMessage::SyncStep1(sv)) => match self.draft().await {
                Ok(draft) => match draft.missing_from(&sv.encode_v1()) {
                    Ok(missing) => Handled::Reply(vec![
                        Message::Sync(SyncMessage::SyncStep2(missing)).encode_v1(),
                    ]),
                    Err(e) => Handled::End(Some(e)),
                },
                Err(_) => Handled::End(None),
            },
            Message::Sync(SyncMessage::SyncStep2(update) | SyncMessage::Update(update)) => {
                self.take(&update).await
            }
            Message::Awareness(update) => {
                let subject: Uuid = self.principal.subject.into();
                let stamped = presence::stamp(update, subject, &self.name);
                let taken = self.room.take_presence(Some(self.id), stamped);
                if !taken.clients.is_empty() {
                    // The sender too: y-websocket's clients take hearing nothing
                    // for 30 seconds, not even their own awareness, as a
                    // dropped connection.
                    self.room
                        .broadcast(None, Message::Awareness(taken.clone()).encode_v1());
                    self.rooms.published_awareness(&self.room, &taken).await;
                }
                Handled::Reply(Vec::new())
            }
            Message::AwarenessQuery => Handled::Reply(
                self.room
                    .presence()
                    .map(|present| Message::Awareness(present).encode_v1())
                    .into_iter()
                    .collect(),
            ),
            Message::Auth(_) | Message::Custom(..) => Handled::Reply(Vec::new()),
        }
    }

    /// Takes an update the client sent, if they may edit.
    async fn take(&mut self, update: &[u8]) -> Handled {
        if kuria_engine::live::is_empty(update) {
            return Handled::Reply(Vec::new());
        }
        if !self.may_edit {
            return Handled::Reply(vec![denied(READ_ONLY)]);
        }
        let caller = Caller {
            actor: self.principal.actor(),
            auth: &self.auth,
            storage: self.rooms.akter().storage(),
            renderer: self.rooms.akter().renderer(),
        };
        let taken = self
            .rooms
            .akter()
            .take_live_update(caller, &self.room.number, update)
            .await;
        match taken {
            Ok(Taken::Unchanged) => Handled::Reply(Vec::new()),
            Ok(Taken::Accepted { .. }) => {
                self.room.edited();
                self.rooms.published_update(&self.room).await;
                Handled::Reply(Vec::new())
            }
            // r[impl live.sync]
            // What undoes it reaches every client, the sender too.
            Ok(Taken::Refused { reason, .. }) => {
                self.room.wake();
                self.rooms.published_update(&self.room).await;
                Handled::Reply(vec![denied(&reason)])
            }
            // r[impl archive.readonly]
            Err(AktError::Forbidden | AktError::Archived(_)) => {
                self.may_edit = false;
                Handled::Reply(vec![denied(READ_ONLY)])
            }
            Err(AktError::NotFound(_)) => Handled::End(None),
            Err(AktError::BadRequest(reason)) => Handled::End(Some(reason)),
            Err(e) => {
                tracing::warn!("taking a live draft's update: {e:?}");
                Handled::End(Some("the update couldn't be taken".to_owned()))
            }
        }
    }

    /// The client left: so did its awareness clients.
    async fn leave(self) {
        if let Some(gone) = self.room.leave(self.id) {
            self.room
                .broadcast(Some(self.id), Message::Awareness(gone.clone()).encode_v1());
            self.rooms.published_awareness(&self.room, &gone).await;
        }
        drop(self.receiver);
        // The room closes once its last client is gone.
        self.room.wake();
    }
}

fn sync_step_1(state_vector: &[u8]) -> Vec<u8> {
    use yrs::updates::decoder::Decode;
    let sv = yrs::StateVector::decode_v1(state_vector).unwrap_or_default();
    Message::Sync(SyncMessage::SyncStep1(sv)).encode_v1()
}

/// An auth message refusing what the client sent, saying why.
fn denied(reason: &str) -> Vec<u8> {
    Message::Auth(Some(reason.to_owned())).encode_v1()
}
