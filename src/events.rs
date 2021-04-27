use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, time::UNIX_EPOCH};
use tokio_tungstenite::tungstenite::Message;

fn unix_epoch() -> u128 {
    let t = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    t.as_millis()
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct HelloEvent {
    payload: String,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct AuthenticateEvent {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct EventPayload {
    pub timestamp: u128,
    pub data: XeonEvent,
}

impl EventPayload {
    pub fn new(ev: XeonEvent) -> EventPayload {
        EventPayload {
            timestamp: unix_epoch(),
            data: ev,
        }
    }
}

impl TryFrom<Message> for EventPayload {
    type Error = String;
    fn try_from(message: Message) -> Result<Self, Self::Error> {
        if message.is_text() {
            match serde_json::from_str::<EventPayload>(&message.to_string()) {
                Ok(v) => Ok(v),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Message event is not text.".to_string())
        }
    }
}

impl TryFrom<EventPayload> for Message {
    type Error = String;
    fn try_from(ev: EventPayload) -> Result<Self, Self::Error> {
        match serde_json::to_string(&ev) {
            Ok(res) => Ok(Message::Text(res)),
            Err(err) => Err(err.to_string()),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub enum XeonEvent {
    Hello(HelloEvent),
    Authenticate(AuthenticateEvent),
}
