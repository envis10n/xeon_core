use crate::events::EventPayload;
use crossbeam_channel::{unbounded, Receiver, Sender};
use futures_util::StreamExt;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt,
};
use log::{debug, error, info};
use mongodb::Client;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::fmt::Display;
use std::sync::Arc;
use tokio::net::ToSocketAddrs;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};
use uuid::Uuid;
pub mod handlers;

#[derive(Clone)]
pub struct WebSocket {
    pub uuid: Uuid,
    pub stream: Arc<(
        RwLock<SplitSink<WebSocketStream<TcpStream>, Message>>,
        RwLock<SplitStream<WebSocketStream<TcpStream>>>,
    )>,
    pub authenticated: bool,
    pub username: Option<String>,
    pub uid: Option<Uuid>,
}

impl WebSocket {
    pub fn new(
        uuid: Uuid,
        stream: &Arc<(
            RwLock<SplitSink<WebSocketStream<TcpStream>, Message>>,
            RwLock<SplitStream<WebSocketStream<TcpStream>>>,
        )>,
    ) -> Self {
        WebSocket {
            uuid,
            stream: stream.clone(),
            authenticated: false,
            username: None,
            uid: None,
        }
    }
    pub async fn send(&self, message: Message) -> Result<(), String> {
        let mut writer = self.stream.0.write().await;
        match writer.send(message).await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    }
}

pub type WebSocketClients = Arc<RwLock<HashMap<Uuid, WebSocket>>>;

pub fn empty_clients() -> WebSocketClients {
    Arc::new(RwLock::new(HashMap::new()))
}

pub async fn send_to(
    clients: WebSocketClients,
    uuid: Uuid,
    message: Message,
) -> Result<(), String> {
    let client_reader = clients.write().await;
    if let Some(client) = client_reader.get(&uuid) {
        client.send(message).await
    } else {
        Err("Client not found.".to_string())
    }
}

pub async fn _send_text_to(
    clients: WebSocketClients,
    uuid: Uuid,
    message: &'static str,
) -> Result<(), String> {
    let client_reader = clients.write().await;
    if let Some(client) = client_reader.get(&uuid) {
        client.send(Message::Text(message.to_string())).await
    } else {
        Err("Client not found.".to_string())
    }
}

pub fn message_text(msg: &'static str) -> Message {
    Message::Text(msg.to_string())
}

pub async fn get_client_state(clients: WebSocketClients, uuid: Uuid) -> WebSocket {
    let cr = clients.read().await;
    let cl = cr.get(&uuid).unwrap();
    cl.clone()
}

pub async fn is_client_authenticated(clients: WebSocketClients, uuid: Uuid) -> bool {
    let cr = clients.read().await;
    let ws = cr.get(&uuid).unwrap();
    ws.authenticated
}

pub async fn broadcast(clients: WebSocketClients, message: Message) {
    let client_reader = clients.write().await;
    for (uuid, client) in client_reader.iter() {
        if client.authenticated {
            match client.send(message.clone()).await {
                Err(err) => {
                    error!("Client {} broadcast error: {}", uuid, err);
                }
                Ok(_) => {}
            }
        }
    }
}

#[derive(Clone)]
pub struct ClientMessage {
    uuid: Uuid,
    event: EventPayload,
}

impl ClientMessage {
    pub fn from_payload(uuid: Uuid, ev: EventPayload) -> Self {
        ClientMessage {
            uuid: uuid,
            event: ev,
        }
    }
}

pub type WebSocketChannel = (Sender<ClientMessage>, Receiver<ClientMessage>);

pub struct WebSocketServer {
    clients: WebSocketClients,
    incoming: WebSocketChannel,
    outgoing: WebSocketChannel,
    db_client: Client,
}

impl WebSocketServer {
    pub fn get_incoming(&self) -> WebSocketChannel {
        (self.outgoing.0.clone(), self.incoming.1.clone())
    }
    pub fn get_outgoing(&self) -> WebSocketChannel {
        (self.incoming.0.clone(), self.outgoing.1.clone())
    }
    pub fn get_clients(&self) -> WebSocketClients {
        self.clients.clone()
    }
    pub fn new(db_client: Client) -> WebSocketServer {
        WebSocketServer {
            clients: empty_clients(),
            incoming: unbounded(),
            outgoing: unbounded(),
            db_client: db_client,
        }
    }
    pub async fn listen<A>(&mut self, addr: A)
    where
        A: ToSocketAddrs + Display,
    {
        let try_socket = TcpListener::bind(&addr).await;
        let listener = try_socket.expect("Failed to bind.");
        info!("Listening on: {}", addr);

        let outer_clients = self.clients.clone();
        let outer_db = self.db_client.clone();
        let outer_incoming = self.incoming.0.clone();

        let ch_clients = outer_clients.clone();
        let _ch_db = outer_db.clone();
        let ch_outgoing = self.outgoing.1.clone();
        tokio::spawn(async move {
            loop {
                let mut events = ch_outgoing.try_iter();
                while let Some(ev) = events.next() {
                    let payload = ev.event.clone();
                    send_to(
                        ch_clients.clone(),
                        ev.uuid.clone(),
                        payload.try_into().unwrap(),
                    )
                    .await
                    .unwrap();
                }
            }
        });

        while let Ok((stream, _)) = listener.accept().await {
            let o_clients = outer_clients.clone();
            let _o_db = outer_db.clone();
            let o_inc = outer_incoming.clone();
            tokio::spawn(async move {
                let uuid = Uuid::new_v4();
                let addr = stream
                    .peer_addr()
                    .expect("Connected stream should have a peer address.");
                info!("Connection from: {}", addr);
                debug!("Connection {} assigned to {}", addr, uuid);

                let ws_stream = accept_async(stream)
                    .await
                    .expect("Error during websocket handshake.");
                info!("{} established WebSocket connection.", uuid);

                let (write, read) = ws_stream.split();

                let streamer = Arc::new((RwLock::new(write), RwLock::new(read)));

                let ws = WebSocket::new(uuid.clone(), &streamer);

                {
                    let mut rwlock = o_clients.write().await;
                    rwlock.insert(uuid.clone(), ws);
                    debug!("Added client {} to clients list.", uuid);
                }

                let inner_stream = streamer.clone();

                let mut reader = inner_stream.1.write().await;

                while let Some(msg_res) = reader.next().await {
                    match msg_res {
                        Ok(msg) => {
                            // Get current state.
                            let wsclient = get_client_state(o_clients.clone(), uuid).await;
                            // Handle Message
                            debug!("Client {} message event: {}", uuid, msg);
                            if msg.is_text() {
                                if let Ok(ev) = EventPayload::try_from(msg.clone()) {
                                    debug!("Received client event: {:?}", ev);
                                    o_inc
                                        .send(ClientMessage::from_payload(uuid.clone(), ev.clone()))
                                        .unwrap();
                                    debug!("Sent client event via incoming send channel.");
                                } else if wsclient.authenticated {
                                    broadcast(
                                        o_clients.clone(),
                                        Message::Text(format!(
                                            "[{}]: {}",
                                            wsclient.username.unwrap_or(uuid.to_string()),
                                            msg.to_string()
                                        )),
                                    )
                                    .await;
                                } else {
                                    send_to(
                                        o_clients.clone(),
                                        uuid,
                                        Message::Text(
                                            "Please authenticate before sending messages."
                                                .to_string(),
                                        ),
                                    )
                                    .await
                                    .unwrap();
                                }
                            } else if msg.is_binary() {
                                info!(
                                    "Client {} message BIN: {}",
                                    uuid,
                                    msg.to_text().unwrap_or(&msg.to_string())
                                );
                            } else if msg.is_close() {
                                info!("Client {} message CLOSE.", uuid);
                                break;
                            }
                        }
                        Err(err) => {
                            info!("Client {} error on read: {}", uuid, err);
                            break;
                        }
                    }
                }

                {
                    let mut rwlock = o_clients.write().await;
                    if let Some(_client) = rwlock.remove(&uuid) {
                        debug!("Client {} removed from client map.", uuid);
                    }
                }
                info!("Client {} disconnected.", uuid);
            });
        }
    }
}
