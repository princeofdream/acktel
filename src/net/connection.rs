use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Closing,
}

#[derive(Debug, Clone)]
pub struct ConnectResult {
    pub success: bool,
    pub error_message: String,
}

pub struct ConnectionInner {
    read_half: Option<OwnedReadHalf>,
    write_half: Option<OwnedWriteHalf>,
    state: ConnectionState,
}

#[derive(Clone)]
pub struct Connection {
    inner: Arc<Mutex<ConnectionInner>>,
    data_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    error_tx: tokio::sync::mpsc::UnboundedSender<String>,
    close_tx: tokio::sync::mpsc::UnboundedSender<()>,
}

impl Connection {
    pub fn new(
        data_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
        error_tx: tokio::sync::mpsc::UnboundedSender<String>,
        close_tx: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConnectionInner {
                read_half: None,
                write_half: None,
                state: ConnectionState::Disconnected,
            })),
            data_tx,
            error_tx,
            close_tx,
        }
    }

    pub async fn connect(&self, hostname: &str, port: u16, timeout_sec: u32) -> ConnectResult {
        {
            let mut inner = self.inner.lock().await;
            inner.state = ConnectionState::Connecting;
        }

        log::info!("Connecting to {}:{}", hostname, port);

        let addr = format!("{}:{}", hostname, port);
        let timeout = Duration::from_secs(timeout_sec as u64);

        match tokio::time::timeout(timeout, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                let (read_half, write_half) = stream.into_split();

                let mut inner = self.inner.lock().await;
                inner.read_half = Some(read_half);
                inner.write_half = Some(write_half);
                inner.state = ConnectionState::Connected;

                log::info!("Connected to {}:{}", hostname, port);
                ConnectResult { success: true, error_message: String::new() }
            }
            Ok(Err(e)) => {
                let mut inner = self.inner.lock().await;
                inner.state = ConnectionState::Disconnected;
                let msg = e.to_string();
                log::error!("Connection failed: {}", msg);
                ConnectResult { success: false, error_message: msg }
            }
            Err(_) => {
                let mut inner = self.inner.lock().await;
                inner.state = ConnectionState::Disconnected;
                let msg = format!("Connection timed out after {}s", timeout_sec);
                log::error!("{}", msg);
                ConnectResult { success: false, error_message: msg }
            }
        }
    }

    pub async fn disconnect(&self) {
        let mut inner = self.inner.lock().await;
        if inner.state == ConnectionState::Disconnected {
            return;
        }
        inner.state = ConnectionState::Closing;
        log::info!("Disconnecting");
        inner.read_half = None;
        inner.write_half = None;
        inner.state = ConnectionState::Disconnected;
    }

    pub async fn is_connected(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.state == ConnectionState::Connected
    }

    pub async fn state(&self) -> ConnectionState {
        let inner = self.inner.lock().await;
        inner.state
    }

    pub async fn send(&self, data: &[u8]) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.state != ConnectionState::Connected {
            return false;
        }
        if let Some(ref mut writer) = inner.write_half {
            match writer.write_all(data).await {
                Ok(()) => true,
                Err(e) => {
                    log::error!("Send failed: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    pub async fn start_read(self) {
        let mut buf = vec![0u8; 4096];
        loop {
            let read_result = {
                let mut inner = self.inner.lock().await;
                if let Some(ref mut reader) = inner.read_half {
                    Some(reader.read(&mut buf).await)
                } else {
                    None
                }
            };

            match read_result {
                Some(Ok(0)) => {
                    log::info!("Connection closed by remote");
                    {
                        let mut inner = self.inner.lock().await;
                        inner.state = ConnectionState::Disconnected;
                    }
                    let _ = self.close_tx.send(());
                    break;
                }
                Some(Ok(n)) => {
                    let data = buf[..n].to_vec();
                    let _ = self.data_tx.send(data);
                }
                Some(Err(e)) => {
                    if e.kind() != std::io::ErrorKind::ConnectionReset
                        && e.kind() != std::io::ErrorKind::UnexpectedEof
                    {
                        log::error!("Read error: {}", e);
                        let _ = self.error_tx.send(e.to_string());
                    } else {
                        log::info!("Connection closed by remote");
                    }
                    {
                        let mut inner = self.inner.lock().await;
                        inner.state = ConnectionState::Disconnected;
                    }
                    let _ = self.close_tx.send(());
                    break;
                }
                None => break,
            }
        }
    }
}
