use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;
use std::sync::Arc;
use std::env;

#[tokio::main]
async fn main() {
    let node_port = env::args().nth(1).expect("No argument provided").parse().expect("Should be valid port");
    let peers = Arc::new(Mutex::new(Vec::new()));
    let (tx, mut rx) = mpsc::channel::<String>(100);

    let listener_peers = Arc::clone(&peers);
    let listener_tx = tx.clone();
    task::spawn(async move {
        listen_for_connections(node_port, listener_peers, listener_tx).await;
    });

    let input_tx = tx.clone();
    let input_peers = Arc::clone(&peers);
    task::spawn(async move {
        handle_user_input(input_peers, input_tx).await;
    });

    while let Some(msg) = rx.recv().await {
        println!("Received: {}", msg);
    }
}

async fn listen_for_connections(port: u16, peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>, tx: mpsc::Sender<String>) {
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("Listening on port {}", port);

    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let stream = Arc::new(Mutex::new(stream));
            println!("New connection accepted");
            let peer_peers = Arc::clone(&peers);
            let peer_tx = tx.clone();
            let mut peers_lock = peer_peers.lock().await;

            let stream_clone = Arc::clone(&stream);
            peers_lock.push(stream_clone);

            drop(peers_lock);
            task::spawn(async move {
                handle_connection(stream, peer_tx).await;
            });
        }
    }
}


async fn handle_connection(stream: Arc<Mutex<TcpStream>>, tx: mpsc::Sender<String>) {
    let mut ste = stream.lock().await;
    let (reader, mut writer) = ste.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => break, 
            Ok(_) => {
                let msg = line.trim().to_string();
                println!("Message received: {}", msg);
                tx.send(msg.clone()).await.unwrap();
            }
            Err(_) => break,
        }
    }
}


async fn send_message(peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>, msg: &str) {
    let mut peers_lock = peers.lock().await;
    for peer in peers_lock.iter_mut() {
        let mut pe = peer.lock().await;
        let _ = pe.write_all(msg.as_bytes()).await;
        let _ = pe.write_all(b"\n").await;
        let _ = pe.flush().await;
    }
}


async fn handle_user_input(peers: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>, tx: mpsc::Sender<String>) {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    loop {
        input.clear();
        reader.read_line(&mut input).await.unwrap();
        let input = input.trim();

        if input.starts_with("connect ") {
            let peer_addr = input.strip_prefix("connect ").unwrap().to_string();
            if let Ok(stream) = TcpStream::connect(&peer_addr).await {
                let stream = Arc::new(Mutex::new(stream));
                println!("Connected to {}", peer_addr);
                let mut peers_lock = peers.lock().await;

                let stream_clone = Arc::clone(&stream);
                peers_lock.push(stream_clone); 
                drop(peers_lock);
                let peer_tx = tx.clone();
                task::spawn(async move {
                    handle_connection(stream, peer_tx).await;
                });
            } else {
                println!("Failed to connect to {}", peer_addr);
            }
        } else {
            send_message(peers.clone(), input).await;
        }
    }
}
