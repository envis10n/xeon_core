use config::WebSocketConfig;
use log::LevelFilter;
use mongodb::Client;
use simple_logger::SimpleLogger;
use std::io::Error;
mod config;
mod events;
mod fs;
mod network;

use network::WebSocketServer;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Init logger
    let logger = SimpleLogger::new();
    #[cfg(debug_assertions)]
    {
        logger.with_level(LevelFilter::Debug).init().unwrap();
        log::debug!("Debug logging enabled.");
    }
    #[cfg(not(debug_assertions))]
    {
        logger.with_level(LevelFilter::Info).init().unwrap();
    }

    // Load configuration
    let xeon_config = config::read_config_file("xeon.toml").await?;

    // Start DB backend
    let mongo_config = xeon_config
        .mongodb_str
        .unwrap_or("mongodb://localhost:27017".to_string());
    let mongod = Client::with_uri_str(&mongo_config).await.unwrap();

    // WebSocket handling
    let websocket_config = xeon_config.websocket.unwrap_or(WebSocketConfig::default());
    let bind_addr = websocket_config.bind_addr();
    let mut ws_server = WebSocketServer::new(mongod.clone());

    // Get WS event channels for use elsewhere.
    let ws_incoming = ws_server.get_incoming();
    let _ws_outgoing = ws_server.get_outgoing();
    let ws_clients = ws_server.get_clients();

    // Move the WS server into an async block to spawn a task.
    let ws_future = async move { ws_server.listen(bind_addr).await };

    match tokio::try_join!(
        tokio::spawn(ws_future),
        tokio::spawn(network::handlers::handle_events(
            ws_clients.clone(),
            mongod.clone(),
            ws_incoming.clone()
        ))
    ) {
        Ok(_) => {}
        Err(_) => {}
    }
    Ok(())
}
