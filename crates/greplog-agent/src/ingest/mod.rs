use anyhow::Result;
use bytes::{Buf, BytesMut};
use greplog_core::gen::{IngestBatch, IngestResponse};
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tokio_util::codec::Decoder;
use tracing::{debug, error, info};

/// 16 MB max frame size to prevent OOM from malicious/corrupt frames
const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

pub struct IngestServer {
    batch_tx: mpsc::Sender<(IngestBatch, bytes::Bytes, oneshot::Sender<IngestResponse>)>,
}

impl IngestServer {
    pub fn new(
        batch_tx: mpsc::Sender<(IngestBatch, bytes::Bytes, oneshot::Sender<IngestResponse>)>,
    ) -> Self {
        Self { batch_tx }
    }

    pub fn spawn(self, config: &super::Config) -> tokio::task::JoinHandle<()> {
        let tcp_port = config.tcp_port;
        let socket_path = config.abs_socket_path();
        let batch_tx = self.batch_tx;

        tokio::spawn(async move {
            let tcp_tx = batch_tx.clone();
            let tcp_handle = tokio::spawn(async move {
                let addr = format!("127.0.0.1:{}", tcp_port);
                let listener = match TcpListener::bind(&addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Failed to bind TCP ingest port {}: {}", addr, e);
                        return;
                    }
                };
                info!("Ingest listening on TCP {}", addr);

                loop {
                    match listener.accept().await {
                        Ok((stream, _peer)) => {
                            let tx = tcp_tx.clone();
                            tokio::spawn(async move {
                                handle_connection(stream, tx).await;
                            });
                        }
                        Err(e) => error!("TCP accept error: {}", e),
                    }
                }
            });

            let uds_tx = batch_tx;
            let uds_handle = tokio::spawn(async move {
                if socket_path.exists() {
                    let _ = tokio::fs::remove_file(&socket_path).await;
                }

                let listener = match UnixListener::bind(&socket_path) {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Failed to bind UDS path {:?}: {}", socket_path, e);
                        return;
                    }
                };
                info!("Ingest listening on UDS {:?}", socket_path);

                loop {
                    match listener.accept().await {
                        Ok((stream, _peer)) => {
                            let tx = uds_tx.clone();
                            tokio::spawn(async move {
                                handle_connection(stream, tx).await;
                            });
                        }
                        Err(e) => error!("UDS accept error: {}", e),
                    }
                }
            });

            let _ = tokio::join!(tcp_handle, uds_handle);
        })
    }
}

/// Handle a single connection: read length‑delimited `IngestBatch` frames,
/// forward them to the writer task, and write back `IngestResponse` frames.
async fn handle_connection<S>(
    stream: S,
    batch_tx: mpsc::Sender<(IngestBatch, bytes::Bytes, oneshot::Sender<IngestResponse>)>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut framed = tokio_util::codec::FramedRead::new(reader, LengthDelimitedCodec::default());

    loop {
        match framed.next().await {
            Some(Ok(bytes)) => {
                let bytes = bytes.freeze();

                let decoded = match IngestBatch::decode(bytes.clone()) {
                    Ok(batch) => batch,
                    Err(e) => {
                        debug!("Failed to decode IngestBatch: {}", e);
                        let resp = IngestResponse {
                            accepted: false,
                            events_count: 0,
                            error: format!("decode_error: {e}"),
                        };
                        let _ = write_response(&mut writer, &resp).await;
                        continue;
                    }
                };

                let (resp_tx, resp_rx) = oneshot::channel();
                if batch_tx.send((decoded, bytes, resp_tx)).await.is_err() {
                    debug!("Writer channel closed, stopping connection handler");
                    break;
                }

                let response = match resp_rx.await {
                    Ok(r) => r,
                    Err(_) => {
                        debug!("Writer dropped the response channel");
                        break;
                    }
                };

                if write_response(&mut writer, &response).await.is_err() {
                    break;
                }
            }
            Some(Err(e)) => {
                debug!("Connection read error: {}", e);
                break;
            }
            None => break,
        }
    }
}

/// Encode an `IngestResponse` as a length‑prefixed frame and write it.
async fn write_response<W: AsyncWrite + Unpin>(
    writer: &mut W,
    response: &IngestResponse,
) -> std::io::Result<()> {
    let encoded = response.encode_to_vec();
    let len = (encoded.len() as u32).to_le_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&encoded).await?;
    Ok(())
}

/// A simple 4-byte little-endian length-delimited codec.
#[derive(Default)]
struct LengthDelimitedCodec {
    next_len: Option<usize>,
}

impl Decoder for LengthDelimitedCodec {
    type Item = BytesMut;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.next_len.is_none() {
            if src.len() < 4 {
                return Ok(None);
            }
            let len = src.get_u32_le() as usize;

            if len > MAX_FRAME_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Frame size {} exceeds maximum of {}", len, MAX_FRAME_SIZE),
                ));
            }

            self.next_len = Some(len);
        }

        if let Some(len) = self.next_len {
            if src.len() < len {
                src.reserve(len - src.len());
                return Ok(None);
            }
            let data = src.split_to(len);
            self.next_len = None;
            return Ok(Some(data));
        }

        Ok(None)
    }
}
