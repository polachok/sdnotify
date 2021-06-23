use crate::{Error, InnerMessage, Message};
use std::env;
use std::path::Path;
use tokio1::net::UnixDatagram;

#[derive(Debug)]
pub struct SdNotify {
    socket: UnixDatagram,
}

impl SdNotify {
    pub fn from_env() -> Result<Self, Error> {
        let sockname = env::var("NOTIFY_SOCKET")?;
        Self::from_path(sockname)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let socket = UnixDatagram::unbound()?;
        socket.connect(path.as_ref())?;
        Ok(SdNotify { socket })
    }

    /// Tells the init system that daemon startup is finished.
    pub async fn notify_ready(&mut self) -> Result<(), std::io::Error> {
        self.state(Message::ready()).await
    }

    /// Passes a single-line status string back to the init system that describes the daemon state.
    pub async fn set_status(&mut self, status: String) -> Result<(), std::io::Error> {
        self.state(Message::status(status)?).await
    }

    /// Tells systemd to update the watchdog timestamp.
    /// This is the keep-alive ping that services need to issue in regular
    /// intervals if WatchdogSec= is enabled for it.
    pub async fn ping_watchdog(&mut self) -> Result<(), std::io::Error> {
        self.state(Message::watchdog()).await
    }

    pub async fn state(&mut self, state: Message) -> Result<(), std::io::Error> {
        match state.0 {
            InnerMessage::Ready => self.socket.send(b"READY=1").await?,
            InnerMessage::Status(status) => {
                self.socket
                    .send(format!("STATUS={}", status).as_bytes())
                    .await?
            }
            InnerMessage::Watchdog => self.socket.send(b"WATCHDOG=1").await?,
        };
        Ok(())
    }
}
