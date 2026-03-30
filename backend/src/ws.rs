use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::broadcast;
use crate::models::{BroadcastTx, SharedState};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State((state, tx)): State<(SharedState, BroadcastTx)>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, tx))
}

async fn handle_socket(socket: WebSocket, state: SharedState, tx: BroadcastTx) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = tx.subscribe();

    // Send current snapshot on connect
    {
        let s = state.read().await;
        let overview = crate::api::build_overview(&s);
        if let Ok(json) = serde_json::to_string(&overview) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(data) => {
                        if sender.send(Message::Text(data.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(d))) => {
                        let _ = sender.send(Message::Pong(d)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}
