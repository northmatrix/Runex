use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::task;

#[derive(Clone, Debug)]
struct Message {
    sender: SocketAddr,
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!(
        "127.0.0.1:{}",
        std::env::args().nth(1).unwrap().parse::<u16>().unwrap()
    );
    let (tx, _rx) = tokio::sync::broadcast::channel::<Message>(100);
    //Listen for incoming connections and handle them
    let tx_clone = tx.clone();
    let listener = task::spawn(async move {
        listen_for_connections(tx_clone, addr).await;
    });
    //Make outgoing connections and handle then
    let _ = listener.await;
    Ok(())
}

async fn listen_for_connections(tx: broadcast::Sender<Message>, addr: String) {
    let listener = TcpListener::bind(&addr).await.unwrap();
    println!("Listening on {}", &addr);
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            create_stream_handler(stream, tx.clone()).await;
        }
    }
}

async fn create_stream_handler(stream: TcpStream, tx: broadcast::Sender<Message>) {
    let socket = stream.peer_addr().unwrap();
    println!("Connection started: {}", socket);
    let (mut reader, mut writer) = stream.into_split();
    // Spawn a new task to handle reading from this peer
    let tx_clone = tx.clone();
    task::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            let n = match reader.read(&mut buf).await {
                Ok(0) => {
                    println!("Connection closed: {}", socket);
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    println!("Failed to read from stream {:?}", e);
                    break;
                }
            };

            let recieved = String::from_utf8_lossy(&buf[..n]).trim().to_string();
            println!("Recieved: {}", recieved);

            let message = Message {
                sender: reader.peer_addr().unwrap(),
                message: recieved,
            };

            if let Err(e) = tx_clone.send(message) {
                println!("Failed to send message to the channel: {:?}", e);
                break;
            }
        }
    });
    // Spawn a new task to handle writing to this peer
    let mut rx = tx.subscribe();
    task::spawn(async move {
        while let Ok(recieved) = rx.recv().await {
            if recieved.sender != writer.peer_addr().unwrap() {
                let _ = writer.write_all(recieved.message.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                let _ = writer.flush().await;
            }
        }
    });
}
