use anyhow::Result;
use bytes::{Bytes, BytesMut};
use prost::Message;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Framed};
use tracing::{error, info, warn};

/// Ingest server — listens on UDS and TCP fallback.
pub struct IngestServer {
    batch_tx: mpsc::Sender<Bytes>,
}

impl IngestServer {
    pub fn new(batch_tx: mpsc::Sender<Bytes>) -> Self {
        Self { batch_tx }
    }

    /// Start both UDS and TCP listeners.
    pub fn spawn(self, config: &super::Config) -> tokio::task::JoinHandle<()> {
        let uds_path = config.abs_socket_path();
        let tcp_port = config.tcp_port;
        let tx1 = self.batch_tx.clone();
        let tx2 = self.batch_tx;

        tokio::spawn(async move {
            // Remove stale socket if any
            let _ = tokio::fs::remove_file(&uds_path).await;

            // UDS listener
            let uds_listener = match UnixListener::bind(&uds_path) {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind UDS at {:?}: {}", uds_path, e);
                    return;
                }
            };
            info!("UDS listening at {:?}", uds_path);

            // TCP fallback listener
            let addr = format!("127.0.0.1:{}", tcp_port);
            let tcp_listener = match TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind TCP at {}: {}", addr, e);
                    return;
                }
            };
            info!("TCP fallback listening on {}", addr);

            // Spawn TCP acceptor
            let tcp_tx = tx2;
            tokio::spawn(async move {
                loop {
                    match tcp_listener.accept().await {
                        Ok((stream, peer)) => {
                            info!("TCP connection from {}", peer);
                            let tx = tcp_tx.clone();
                            tokio::spawn(handle_connection(stream, tx));
                        }
                        Err(e) => error!("TCP accept error: {}", e),
                    }
                }
            });

            // UDS acceptor
            loop {
                match uds_listener.accept().await {
                    Ok((stream, _)) => {
                        let tx = tx1.clone();
                        tokio::spawn(handle_connection(stream, tx));
                    }
                    Err(e) => error!("UDS accept error: {}", e),
                }
            }
        })
    }
}

/// Wraps a raw stream into a length-delimited protobuf frame decoder
/// and forwards each complete batch into the channel.
async fn handle_connection<S>(stream: S, batch_tx: mpsc::Sender<Bytes>)
where
    S: AsyncRead + AsyncWriteExt + Unpin + Send + 'static,
{
    let mut framed = LengthDelimitedCodec::default().framed(stream);

    loop {
        match framed.next().await {
            Some(Ok(buf)) => {
                if batch_tx.send(buf).await.is_err() {
                    warn!("Batch channel closed, dropping connection");
                    break;
                }
            }
            Some(Err(e)) => {
                error!("Frame decode error: {}", e);
                break;
            }
            None => break, // stream closed
        }
    }
}

/// Simple length-delimited framing: 4-byte big-endian length prefix + payload.
#[derive(Default)]
struct LengthDelimitedCodec {
    state: DecodeState,
}

enum DecodeState {
    Head,
    Data(usize),
}

impl Default for DecodeState {
    fn default() -> Self {
        DecodeState::Head
    }
}

impl Decoder for LengthDelimitedCodec {
    type Item = Bytes;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Bytes>, Self::Error> {
        loop {
            match self.state {
                DecodeState::Head => {
                    if src.len() < 4 {
                        return Ok(None);
                    }
                    let len = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;
                    src.advance(4);
                    self.state = DecodeState::Data(len);
                }
                DecodeState::Data(len) => {
                    if src.len() < len {
                        return Ok(None);
                    }
                    let data = src.split_to(len).freeze();
                    self.state = DecodeState::Head;
                    return Ok(Some(data));
                }
            }
        }
    }
}

// Helper: implement Framed-compatible Next for our wrapped type.
use futures::StreamExt;

impl<S> futures::Stream for Framed<S, LengthDelimitedCodec>
where
    S: AsyncRead + Unpin,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.as_mut().poll_next_unpin(cx) // Delegates to StreamExt via decode
    }
}
