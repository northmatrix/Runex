use std::sync::Arc;
use tokio::task;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peers: Arc<Mutex<Vec<Arc<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
    let (tx, _rx) = broadcast::channel::<String>(100);

    let peers_clone = Arc::clone(&peers);
    let tx_clone = tx.clone();
    task::spawn(async move { listen_for_connections(peers_clone, tx_clone).await });

    let mut rx = tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        println!("Received: {}", msg);
    }
    Ok(())
}

async fn listen_for_connections(
    peers: Arc<Mutex<Vec<Arc<TcpStream>>>>,
    tx: broadcast::Sender<String>,
) {
    let listener = TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("Binding to port failed");
    println!("Listening on port 127.0.0.1:8080");
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let stream = Arc::new(stream);
            let peers_clone = Arc::clone(&peers);
            let peer_tx = tx.clone();

            let mut peers_lock = peers_clone.lock().await;
            let stream_clone = Arc::clone(&stream);

            peers_lock.push(stream_clone);
            drop(peers_lock);
            task::spawn(async move {
                handle_connection(stream, peer_tx).await;
            });
        }
    }
}

async fn handle_connection(stream: Arc<TcpStream>, peer_tx: broadcast::Sender<String>) {
    todo!()
}
