//! IPC client for communicating with the daemon

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::config::Settings;
use crate::daemon::ipc::{
    deserialize_response, serialize_request, DaemonRequest, DaemonResponse,
};

/// Client for communicating with the daemon
pub struct DaemonClient {
    stream: UnixStream,
}

impl DaemonClient {
    /// Connect to the daemon
    pub async fn connect(settings: &Settings) -> Result<Self> {
        let socket_path = settings.socket_path();

        let stream = UnixStream::connect(&socket_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to connect to daemon at {:?}. Is the daemon running? Try: minutes daemon start",
                    socket_path
                )
            })?;

        Ok(Self { stream })
    }

    /// Send a request and wait for response
    pub async fn send(&mut self, request: DaemonRequest) -> Result<DaemonResponse> {
        let stream = &mut self.stream;

        // Serialize and send request
        let bytes = serialize_request(&request);
        stream.write_all(&bytes).await?;

        // Read response length
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;

        // Read response body
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).await?;

        // Deserialize response
        let response = deserialize_response(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

        Ok(response)
    }
}
