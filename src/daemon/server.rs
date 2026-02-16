//! Unix socket IPC server for daemon communication

use anyhow::Result;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::daemon::ipc::{deserialize_request, serialize_response, DaemonRequest, DaemonResponse};

/// Command channel for the server
pub type CommandSender = mpsc::Sender<(DaemonRequest, mpsc::Sender<DaemonResponse>)>;
pub type CommandReceiver = mpsc::Receiver<(DaemonRequest, mpsc::Sender<DaemonResponse>)>;

/// IPC server that listens on a Unix socket
pub struct IpcServer {
    socket_path: PathBuf,
    listener: Option<UnixListener>,
}

impl IpcServer {
    /// Create a new IPC server
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            listener: None,
        }
    }

    /// Start listening on the socket
    pub async fn start(&mut self) -> Result<()> {
        // Remove stale socket file if it exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        info!("IPC server listening on {:?}", self.socket_path);
        self.listener = Some(listener);

        Ok(())
    }

    /// Run the server, forwarding commands to the handler
    pub async fn run(&mut self, cmd_tx: CommandSender) -> Result<()> {
        let listener = self.listener.take().expect("Server not started");

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let tx = cmd_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, tx).await {
                            error!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket file
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// Handle a single client connection
async fn handle_connection(mut stream: UnixStream, cmd_tx: CommandSender) -> Result<()> {
    debug!("New client connection");

    loop {
        // Read message length (4 bytes, little-endian)
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!("Client disconnected");
                break;
            }
            Err(e) => return Err(e.into()),
        }

        let len = u32::from_le_bytes(len_buf) as usize;

        if len > 1024 * 1024 {
            warn!("Message too large: {} bytes", len);
            break;
        }

        // Read message body
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).await?;

        // Deserialize request
        let request = match deserialize_request(&body) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to deserialize request: {}", e);
                let response = DaemonResponse::Error {
                    message: format!("Invalid request: {}", e),
                };
                let bytes = serialize_response(&response);
                stream.write_all(&bytes).await?;
                continue;
            }
        };

        debug!("Received request: {:?}", request);

        // Check for shutdown before sending to handler
        let is_shutdown = matches!(request, DaemonRequest::Shutdown);

        // Send to handler and wait for response
        let (resp_tx, mut resp_rx) = mpsc::channel(1);
        cmd_tx.send((request, resp_tx)).await?;

        let response = resp_rx.recv().await.unwrap_or(DaemonResponse::Error {
            message: "Handler closed".to_string(),
        });

        // Send response
        let bytes = serialize_response(&response);
        stream.write_all(&bytes).await?;

        // If shutdown was requested, close connection
        if is_shutdown {
            break;
        }
    }

    Ok(())
}
