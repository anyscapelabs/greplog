use std::io::Write;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const DEFAULT_SOCKET_PATH: &str = ".greplog/greplog.sock";
const DEFAULT_TCP_HOST: &str = "127.0.0.1";
const DEFAULT_TCP_PORT: u16 = 4318;
const DEFAULT_RECONNECT_DELAY: Duration = Duration::from_secs(5);

#[allow(dead_code)]
enum Cmd {
    Send(Vec<u8>),
    #[allow(dead_code)]
    Shutdown,
}

pub struct Transport {
    tx: Sender<Cmd>,
}

impl Transport {
    pub fn new(
        socket_path: Option<&str>,
        tcp_host: Option<&str>,
        tcp_port: Option<u16>,
        reconnect_delay: Option<Duration>,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();

        let sock = socket_path.unwrap_or(DEFAULT_SOCKET_PATH).to_string();
        let host = tcp_host.unwrap_or(DEFAULT_TCP_HOST).to_string();
        let port = tcp_port.unwrap_or(DEFAULT_TCP_PORT);
        let delay = reconnect_delay.unwrap_or(DEFAULT_RECONNECT_DELAY);

        thread::spawn(move || run(rx, &sock, &host, port, delay));

        Transport { tx }
    }

    pub fn send(&self, data: Vec<u8>) {
        let _ = self.tx.send(Cmd::Send(data));
    }

    #[allow(dead_code)]
    pub fn shutdown(&self) {
        let _ = self.tx.send(Cmd::Shutdown);
    }
}

fn run(rx: Receiver<Cmd>, sock_path: &str, tcp_host: &str, tcp_port: u16, delay: Duration) {
    let mut pending: Vec<Vec<u8>> = Vec::new();
    let mut warned = false;

    loop {
        if !pending.is_empty() {
            if let Some(mut conn) = connect(sock_path, tcp_host, tcp_port) {
                pending.retain(|frame| write_frame(&mut conn, frame).is_err());
            } else if !warned {
                warned = true;
                eprintln!("[greplog] Agent not found. Run 'greplog dev' to capture logs.");
            }
        }

        match rx.recv_timeout(delay) {
            Ok(Cmd::Send(data)) => {
                pending.push(data);
            }
            Ok(Cmd::Shutdown) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn connect(sock_path: &str, tcp_host: &str, tcp_port: u16) -> Option<Box<dyn Write + Send>> {
    if let Ok(stream) = UnixStream::connect(sock_path) {
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
        Some(Box::new(stream))
    } else if let Ok(stream) = TcpStream::connect((tcp_host, tcp_port)) {
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
        Some(Box::new(stream))
    } else {
        None
    }
}

fn write_frame(conn: &mut dyn Write, data: &[u8]) -> Result<(), ()> {
    let len = data.len() as u32;
    let header = len.to_le_bytes();
    conn.write_all(&header).map_err(|_| ())?;
    conn.write_all(data).map_err(|_| ())?;
    conn.flush().map_err(|_| ())?;
    Ok(())
}
