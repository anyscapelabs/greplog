//! Integration tests against a hand-rolled HTTP fake: batching, retry
//! semantics, queue caps, and the wire record shape.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use greplog::Client;

/// Serves N responses (status configurable per slot), capturing request bodies.
struct FakeServer {
    url: String,
    received: Arc<Mutex<Vec<serde_json::Value>>>,
    statuses: Arc<Mutex<Vec<u16>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    fn start(statuses: Vec<u16>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://127.0.0.1:{}", listener.local_addr().expect("addr").port());
        let received = Arc::new(Mutex::new(Vec::new()));
        let status_queue = Arc::new(Mutex::new(statuses));

        let handle = std::thread::spawn({
            let received = Arc::clone(&received);
            let status_queue = Arc::clone(&status_queue);
            move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let received = Arc::clone(&received);
                let status_queue = Arc::clone(&status_queue);
                std::thread::spawn(move || {
                        if let Some(body) = read_request(&stream) {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
                                received.lock().unwrap().push(parsed);
                            }
                            let status = status_queue.lock().unwrap().pop().unwrap_or(200);
                            let response = format!("HTTP/1.1 {status} IGNORED\r\nContent-Length: 0\r\n\r\n");
                            let mut stream = stream;
                            let _ = stream.write_all(response.as_bytes());
                        }
                    });
            }
        }
        });

        Self { url, received, statuses: status_queue, handle: Some(handle) }
    }

    fn wait_for(&self, count: usize, timeout: Duration) -> Vec<serde_json::Value> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let snapshot = self.received.lock().unwrap().clone();
            if snapshot.len() >= count || std::time::Instant::now() > deadline {
                return snapshot;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        // Dropping the listener would require moving it out; tests are
        // short-lived enough that leaking the accept thread is fine.
        self.handle.take();
    }
}

fn read_request(stream: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let trimmed = header.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok()?;
        }
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).ok()?;
    Some(String::from_utf8_lossy(&body).into_owned())
}

#[test]
fn batch_size_triggers_one_post_with_the_full_record_shape() {
    let server = Arc::new(FakeServer::start(vec![200]));

    let client = Client::new(
        "test-svc",
        &server.url,
        3,
        Duration::from_secs(60),
        10_000,
    );
    for index in 0..3 {
        client.track(
            "INFO",
            format!("event {index}"),
            Some(serde_json::json!({ "order_id": index })),
        );
    }
    let batches = server.wait_for(1, Duration::from_secs(5));
    assert_eq!(batches.len(), 1);

    let rows = batches[0].as_array().expect("batch is an array");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["message"], "event 0");
    assert_eq!(rows[0]["level"], "INFO");
    assert_eq!(rows[0]["service"], "test-svc");
    assert!(rows[0]["timestamp_us"].is_u64());
    assert_eq!(rows[0]["trace_id"], serde_json::Value::Null);
    assert_eq!(rows[0]["raw_body"], serde_json::json!({ "order_id": 0 }).to_string());
    drop(server.statuses.lock().unwrap());
}

#[test]
fn trace_id_moves_to_its_own_field_and_out_of_raw_body() {
    let server = Arc::new(FakeServer::start(vec![200]));
    let client = Client::new("svc", &server.url, 1, Duration::from_secs(60), 100);
    client.track("ERROR", "boom", Some(serde_json::json!({ "trace_id": "job_9", "code": 7 })));

    let batches = server.wait_for(1, Duration::from_secs(5));
    let row = &batches[0][0];
    assert_eq!(row["trace_id"], "job_9");
    assert_eq!(row["raw_body"], serde_json::json!({ "code": 7 }).to_string());
}

#[test]
fn a_500_requeues_and_resends_the_batch() {
    let server = Arc::new(FakeServer::start(vec![200, 500])); // statuses pop from the back: 500 first, then 200
    let client = Client::new("svc", &server.url, 1, Duration::from_secs(60), 100);
    client.track("INFO", "resend me", None);

    // First attempt gets the 500 and requeues; waking the client flushes again.
    let first = server.wait_for(1, Duration::from_secs(5));
    assert_eq!(first[0][0]["message"], "resend me");
    client.flush();

    let all = server.wait_for(2, Duration::from_secs(5));
    assert_eq!(all.len(), 2, "the rejected batch must come back around");
    assert_eq!(all[1][0]["message"], "resend me");
}

#[test]
fn queue_cap_drops_oldest_records() {
    let server = Arc::new(FakeServer::start(vec![200]));
    let client = Client::new("svc", &server.url, 100, Duration::from_secs(60), 5);
    for index in 0..8 {
        client.track("INFO", format!("m{index}"), None);
    }
    assert_eq!(client.dropped_count(), 3);

    client.flush();
    let batches = server.wait_for(1, Duration::from_secs(5));
    let messages: Vec<&str> = batches[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["message"].as_str().unwrap())
        .collect();
    assert_eq!(messages, vec!["m3", "m4", "m5", "m6", "m7"]);
}
