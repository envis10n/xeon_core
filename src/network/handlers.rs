use super::{WebSocketChannel, WebSocketClients};
use crate::{
    events::{EventPayload, XeonEvent},
    network::ClientMessage,
};
use log::debug;
use mongodb::Client;

pub async fn handle_events(
    clients: WebSocketClients,
    db_client: Client,
    channel: WebSocketChannel,
) {
    let outer_clients = clients.clone();
    let outer_db = db_client.clone();
    let outer_ch = channel.clone();
    debug!("Starting event handler.");
    loop {
        let o_clients = outer_clients.clone();
        let _o_db = outer_db.clone();
        let o_ch = outer_ch.clone();
        let mut rec = o_ch.1.try_iter();
        while let Some(cev) = rec.next() {
            let uuid = cev.uuid.clone();
            let ev = cev.event.clone();
            debug!("Handling event for {}: {:?}", uuid, ev);
            match ev.data {
                XeonEvent::Authenticate(_auth) => {
                    // Trying to authenticate.
                    if !super::is_client_authenticated(o_clients.clone(), uuid).await {
                        {
                            let mut cwriter = o_clients.write().await;
                            let wsr = cwriter.get_mut(&uuid).unwrap();
                            wsr.authenticated = true;
                            wsr.username = Some(_auth.username);
                        }
                        o_ch.0
                            .send(ClientMessage::from_payload(
                                uuid.clone(),
                                EventPayload::print("Successfully authenticated."),
                            ))
                            .unwrap();
                    }
                }
                _ => {}
            }
        }
    }
}
