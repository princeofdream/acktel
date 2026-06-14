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
    state: ConnectionState,
}

pub struct Connection {
    inner: Arc<Mutex<ConnectionInner>>,
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    data_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    error_tx: tokio::sync::mpsc::UnboundedSender<String>,
    close_tx: tokio::sync::mpsc::UnboundedSender<()>,
}

impl Clone for Connection {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            writer: self.writer.clone(),
            data_tx: self.data_tx.clone(),
            error_tx: self.error_tx.clone(),
            close_tx: self.close_tx.clone(),
        }
    }
}

impl Connection {
    pub fn new(
        data_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
        error_tx: tokio::sync::mpsc::UnboundedSender<String>,
        close_tx: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConnectionInner {
                state: ConnectionState::Disconnected,
            })),
            writer: Arc::new(Mutex::new(None)),
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

                {
                    let mut inner = self.inner.lock().await;
                    inner.state = ConnectionState::Connected;
                }
                {
                    let mut w = self.writer.lock().await;
                    *w = Some(write_half);
                }

                // Start read task immediately with the read_half
                let data_tx = self.data_tx.clone();
                let error_tx = self.error_tx.clone();
                let close_tx = self.close_tx.clone();
                let inner = self.inner.clone();

                tokio::spawn(async move {
                    Self::read_loop(read_half, data_tx, error_tx, close_tx, inner).await;
                });

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

    async fn read_loop(
        mut reader: OwnedReadHalf,
        data_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
        error_tx: tokio::sync::mpsc::UnboundedSender<String>,
        close_tx: tokio::sync::mpsc::UnboundedSender<()>,
        inner: Arc<Mutex<ConnectionInner>>,
    ) {
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => {
                    log::info!("Connection closed by remote");
                    {
                        let mut i = inner.lock().await;
                        i.state = ConnectionState::Disconnected;
                    }
                    let _ = close_tx.send(());
                    break;
                }
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    log::debug!("Connection read: {} bytes", n);
                    let _ = data_tx.send(data);
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::ConnectionReset
                        && e.kind() != std::io::ErrorKind::UnexpectedEof
                    {
                        log::error!("Read error: {}", e);
                        let _ = error_tx.send(e.to_string());
                    } else {
                        log::info!("Connection closed by remote");
                    }
                    {
                        let mut i = inner.lock().await;
                        i.state = ConnectionState::Disconnected;
                    }
                    let _ = close_tx.send(());
                    break;
                }
            }
        }
    }

    pub async fn disconnect(&self) {
        {
            let mut inner = self.inner.lock().await;
            if inner.state == ConnectionState::Disconnected {
                return;
            }
            inner.state = ConnectionState::Closing;
            log::info!("Disconnecting");
        }
        {
            let mut w = self.writer.lock().await;
            *w = None;
        }
        {
            let mut inner = self.inner.lock().await;
            inner.state = ConnectionState::Disconnected;
        }
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
        let inner = self.inner.lock().await;
        if inner.state != ConnectionState::Connected {
            return false;
        }
        drop(inner);

        let mut w = self.writer.lock().await;
        if let Some(ref mut writer) = *w {
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
}
