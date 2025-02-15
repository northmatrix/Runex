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
        print!("Received: {}", msg);
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
    println!("Listening on 127.0.0.1:8080");
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let stream = Arc::new(Mutex::new(stream));
            let peers_clone = Arc::clone(&peers);
            let peer_tx_clone = tx.clone();

            let mut peers_clone_lock = peers_clone.lock().await;
            let stream_clone = Arc::clone(&stream);
            peers_clone_lock.push(stream_clone);
            drop(peers_clone_lock);
            //drop(stream_clone); Stream clone has been moved
            
            task::spawn(async move {
                handle_connection(stream, peer_tx_clone,peers_clone).await;
            });
        }
    }
}

async fn handle_connection(stream: Arc<Mutex<TcpStream>>, peer_tx: broadcast::Sender<String>,peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>) {
    let peer_tx_clone = peer_tx.clone();
    let reader_clone = Arc::clone(&stream);
    let read_task = tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            let mut reader_lock = reader_clone.lock().await;
            let _ = reader_lock.read(&mut buf).await;
            drop(reader_lock);
            let data = String::from_utf8_lossy(&buf).to_string();
            peer_tx_clone.send(data.trim().to_string()).expect("Error");
           buf.fill(0);
        }
    });

    let write_task = tokio::spawn(async move {
        loop { 
            broadcast_message_to_peers(&peers, &stream, "Emergency broadcast").await;
        }
    });

    let _ = tokio::join!(read_task, write_task);
}


async fn broadcast_message_to_peers(peers: &Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>,stream: &Arc<Mutex<TcpStream>>,msg: &str) {
    let peers_lock = peers.lock().await;
    for x in peers_lock.iter().filter(|p| Arc::ptr_eq(stream, p.to_owned()) ) {
        let mut x_lock = x.lock().await;
        x_lock.write_all(msg.as_bytes()).await.expect("err");
        x_lock.flush().await.expect("msg");
        drop(x_lock);
    }
    drop(peers_lock);
}