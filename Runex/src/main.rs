use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::task;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>> = Arc::new(Mutex::new(Vec::new()));
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
    peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>,
    tx: broadcast::Sender<String>,
) {
    let listener = TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("Binding to port failed");
    println!("Listening on port 127.0.0.1:8080");
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let stream = Arc::new(Mutex::new(stream));
            let peers_clone = Arc::clone(&peers);
            let peer_tx = tx.clone();

            let mut peers_lock = peers_clone.lock().await;
            let stream_clone = Arc::clone(&stream);

            peers_lock.push(stream_clone);
            drop(peers_lock);
            task::spawn(async move {
                handle_connection(stream, peer_tx,peers_clone).await;
            });
        }
    }
}

async fn handle_connection(stream: Arc<Mutex<TcpStream>>, peer_tx: broadcast::Sender<String>,peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>) {
    let reader = Arc::clone(&stream);
    let writer = Arc::clone(&stream);

    let read_task = tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            let mut reader_lock = reader.lock().await;
            let _ = reader_lock.read(&mut buf).await;
            drop(reader_lock);
            let data = String::from_utf8_lossy(&buf).to_string();
            peer_tx.send(data).expect("Error");
            buf.fill(0);
        }
    });


    let write_task = tokio::spawn(async move {
        loop {
            broadcast_message_to_peers(&writer, &peers, "HELLO").await;
        } 
    });

    let _ = tokio::join!(read_task, write_task);
}

async fn send_message(stream: &Arc<Mutex<TcpStream>>,msg: &str) {
    let stream_lock = stream.lock();
    stream_lock.await.write_all(msg.as_bytes()).await;
}

async fn broadcast_message_to_peers(stream: &Arc<Mutex<TcpStream>>, peers: &Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>, msg: &str) {
    let peers_lock = peers.lock();
    for peer in peers_lock.await.iter() {
        let peer_lock = peer.lock().await;
        if peer_lock.peer_addr().expect("Err") != stream.lock().await.peer_addr().expect("Err") {
            send_message(&stream, msg);
        }
    }
}