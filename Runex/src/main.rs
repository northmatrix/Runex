use std::env::args;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peers: Arc<Mutex<Vec<OwnedWriteHalf>>> = Arc::new(Mutex::new(Vec::new()));
    let peers_clone = Arc::clone(&peers);
    listen_for_connections(peers_clone).await;
    Ok(())
}

async fn listen_for_connections(peers: Arc<Mutex<Vec<OwnedWriteHalf>>>) {
    let port = args().nth(1).unwrap();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(addr)
        .await
        .expect("Binding to port failed");
    println!("Listening on 127.0.0.1:8080.");
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let (reader, writer) = stream.into_split();
            let peers_clone = Arc::clone(&peers);
            peers_clone.lock().await.push(writer);

            // Spawn a new task to handle reading from this peer
            task::spawn(async move {
                handle_peer(reader, peers_clone).await;
            });
        }
    }
}

async fn handle_peer(mut reader: OwnedReadHalf, peers: Arc<Mutex<Vec<OwnedWriteHalf>>>) {
    println!(
        "Connected: {}, NoClientsRemaining: {}",
        reader.peer_addr().unwrap(),
        peers.lock().await.len()
    );
    let mut buf = [0; 1024];
    let socket = reader.peer_addr().unwrap();
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => {
                println!("Disconnected {}", socket);
                let mut peers_lock = peers.lock().await;
                peers_lock.retain(|peer| peer.peer_addr().is_ok());
                break;
            }
            Ok(n) => {
                let data = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                if data.starts_with("connect") {
                    if let Some(address) = data.strip_prefix("connect ") {
                        match TcpStream::connect(address).await {
                            Ok(stream) => {
                                let (reader, writer) = stream.into_split();
                                let mut peers_lock = peers.lock().await;
                                peers_lock.push(writer);
                                drop(peers_lock);
                                let peers_clone = Arc::clone(&peers);
                                task::spawn(handle_peer(reader, peers_clone));
                            }
                            Err(e) => {
                                eprintln!("Failed to connect to {}: {}", address, e);
                            }
                        }
                    }
                    // so suppose this is an ip then how can i connect to it by adding it to peers
                    // list and setting up a handle peer thread for  it
                }
                broadcast_message_to_peers(peers.clone(), &data).await;
            }
            Err(_) => break,
        }
        buf.fill(0);
    }
}

async fn broadcast_message_to_peers(peers: Arc<Mutex<Vec<OwnedWriteHalf>>>, msg: &str) {
    println!("Broadcasting message \"{}\" to all.", msg);
    let mut peer_lock = peers.lock().await;
    for p in peer_lock.iter_mut() {
        let _ = p.write_all(msg.as_bytes()).await;
        let _ = p.flush().await;
    }
}
