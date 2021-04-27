use super::{WebSocketChannel, WebSocketClients};
use crate::events::XeonEvent;
use log::debug;
use mongodb::Client;

pub async fn handle_events(
    clients: WebSocketClients,
    db_client: Client,
    channel: WebSocketChannel,
) {
    let o_clients = clients.clone();
    let _o_db = db_client.clone();
    let o_ch = channel.clone();
    debug!("Starting event handler.");
    loop {
        let mut rec = o_ch.1.try_iter();
        while let Some(cev) = rec.next() {
            let uuid = cev.uuid.clone();
            let ev = cev.event.clone();
            debug!("Handling event for {}: {:?}", uuid, ev);
            match ev.data {
                XeonEvent::Authenticate(_auth) => {
                    // Trying to authenticate.
                    if !super::is_client_authenticated(o_clients.clone(), uuid).await {
                        let mut cwriter = o_clients.write().await;
                        let wsr = cwriter.get_mut(&uuid).unwrap();
                        wsr.authenticated = true;
                        wsr.username = Some(_auth.username);
                        wsr.send(super::message_text("Authenticated successfully."))
                            .await
                            .unwrap();
                    }
                }
                _ => {}
            }
        }
    }
}
