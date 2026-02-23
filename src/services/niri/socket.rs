use std::env;
use std::path::Path;

use futures::lock;
use gtk4::prelude::*;
use gtk4::{gio, glib};

use super::ipc;
pub use ipc::SOCKET_PATH_ENV;

/// Helper for async (Gio) communication over the niri socket.
///
/// See also [niri_ipc::socket::Socket].
#[derive(Default)]
pub struct Socket {
    conn: lock::Mutex<Option<SocketConn>>,
}

struct SocketConn {
    conn: gio::SocketConnection,
    sender: gio::OutputStream,
    receiver: gio::DataInputStream,
}

impl Socket {
    /// Creates a [gio::SocketConnection] for [SOCKET_PATH_ENV].
    pub async fn connect() -> anyhow::Result<gio::SocketConnection> {
        let socket_path =
            env::var_os(SOCKET_PATH_ENV).ok_or_else(|| anyhow::anyhow!("env SOCKET_PATH_ENV not found"))?;
        Self::connect_to(socket_path).await
    }

    /// Creates a [gio::SocketConnection] for the given [Path].
    pub async fn connect_to(path: impl AsRef<Path>) -> anyhow::Result<gio::SocketConnection> {
        let connection = gio::SocketClient::new()
            .connect_future(&gio::UnixSocketAddress::new(path.as_ref()))
            .await?;
        Ok(connection)
    }

    /// Sets the socket connection.
    pub async fn set_conn(&self, conn: gio::SocketConnection) {
        let mut mu = self.conn.lock().await;
        mu.replace(SocketConn::new(conn));
    }

    /// Sends a [ipc::Request] to niri socket.
    ///
    /// A [gio::SocketConnection] will be created if not present.
    pub async fn send(&self, request: ipc::Request) -> anyhow::Result<ipc::Reply> {
        let mut mu = self.conn.lock().await;
        if mu.is_none() {
            mu.replace(SocketConn::new(Self::connect().await?));
        }
        let conn = mu.as_ref().unwrap();

        let mut buf = serde_json::to_vec(&request)?;
        buf.push(b'\n');
        conn.sender
            .write_bytes_future(&glib::Bytes::from_owned(buf), glib::Priority::DEFAULT)
            .await?;

        let line = conn.receiver.read_line_future(glib::Priority::DEFAULT).await?;
        Ok(serde_json::from_slice(&line.unwrap_or_default())?)
    }

    /// Requests event stream from niri socket,
    /// and returns an `AsyncFnMut() -> Result<Option<Event>>`,
    /// where `None` is returned on deserialization failed.
    ///
    /// See also [niri_ipc::socket::Socket::read_events].
    pub async fn read_events(self) -> impl AsyncFnMut() -> anyhow::Result<Option<ipc::Event>> {
        let mut mu = self.conn.lock().await;
        let conn = mu.take().unwrap();
        let _ = conn.sender.close_future(glib::Priority::DEFAULT).await;

        async move || {
            // Keeps `SocketConnection.conn` alive.
            let _ = &conn.conn;
            let line = conn.receiver.read_line_future(glib::Priority::DEFAULT).await?;
            Ok(serde_json::from_slice(&line.unwrap_or_default()).ok())
        }
    }
}

impl SocketConn {
    fn new(conn: gio::SocketConnection) -> Self {
        Self {
            conn: conn.clone(),
            sender: conn.output_stream(),
            receiver: gio::DataInputStream::new(&conn.input_stream()),
        }
    }
}
