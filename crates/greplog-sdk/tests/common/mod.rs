use greplog::gen::IngestBatch;
use prost::Message;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn start_test_server() -> (u16, mpsc::Receiver<IngestBatch>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_connection(stream, tx);
        }
    });

    (port, rx)
}

fn handle_connection(mut stream: TcpStream, tx: mpsc::Sender<IngestBatch>) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut buf = [0u8; 4];

    loop {
        if stream.read_exact(&mut buf).is_err() {
            break;
        }
        let len = u32::from_le_bytes(buf) as usize;
        let mut payload = vec![0u8; len];
        if stream.read_exact(&mut payload).is_err() {
            break;
        }
        if let Ok(batch) = IngestBatch::decode(&payload[..]) {
            let _ = tx.send(batch);
        }
    }
}

pub fn wait_for_frames(rx: &mpsc::Receiver<IngestBatch>, count: usize) -> Vec<IngestBatch> {
    let mut batches = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while batches.len() < count && std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(batch) => batches.push(batch),
            Err(_) => continue,
        }
    }

    batches
}


