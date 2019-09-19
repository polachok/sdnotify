use std::env;
use std::os::unix::net::UnixDatagram;
use std::path::Path;

#[cfg(feature = "async_io")]
pub mod async_io;

pub struct Message(InnerMessage);

impl Message {
    /// Tells the init system that daemon startup is finished.
    pub fn ready() -> Self {
        Message(InnerMessage::Ready)
    }

    /// Passes a single-line status string back to the init system that describes the daemon state.
    pub fn status(status: String) -> Result<Self, std::io::Error> {
        if status.as_bytes().iter().find(|x| **x == b'\n').is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "newline not allowed",
            ));
        }
        Ok(Message(InnerMessage::Status(status)))
    }

    /// Tells systemd to update the watchdog timestamp.
    /// This is the keep-alive ping that services need to issue in regular
    /// intervals if WatchdogSec= is enabled for it.
    pub fn ping_watchdog() -> Self {
        Message(InnerMessage::Watchdog)
    }
}

enum InnerMessage {
    Ready,
    Status(String),
    Watchdog,
}

#[derive(Debug)]
pub enum Error {
    NoSocket,
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSocket => write!(f, "NOTIFY_SOCKET variable not set"),
            Error::Io(err) => write!(f, "{}", err),
        }
    }
}

impl From<env::VarError> for Error {
    fn from(_: env::VarError) -> Error {
        Error::NoSocket
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl std::error::Error for Error {}

pub struct SdNotify(UnixDatagram);

impl SdNotify {
    pub fn from_env() -> Result<Self, Error> {
        let sockname = env::var("NOTIFY_SOCKET")?;
        Self::from_path(sockname)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let socket = UnixDatagram::unbound()?;
        socket.connect(path)?;
        Ok(SdNotify(socket))
    }

    /// Tells the init system that daemon startup is finished.
    pub fn ready(&self) -> Result<(), std::io::Error> {
        self.state(Message::ready())
    }

    /// Passes a single-line status string back to the init system that describes the daemon state.
    pub fn status(&self, status: String) -> Result<(), std::io::Error> {
        self.state(Message::status(status)?)
    }

    /// Tells systemd to update the watchdog timestamp.
    /// This is the keep-alive ping that services need to issue in regular
    /// intervals if WatchdogSec= is enabled for it.
    pub fn ping_watchdog(&self) -> Result<(), std::io::Error> {
        self.state(Message::ping_watchdog())
    }

    fn state(&self, state: Message) -> Result<(), std::io::Error> {
        match state.0 {
            InnerMessage::Ready => self.0.send(b"READY=1")?,
            InnerMessage::Status(status) => self.0.send(format!("STATUS={}", status).as_bytes())?,
            InnerMessage::Watchdog => self.0.send(b"WATCHDOG=1")?,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ok() {
        use super::*;

        let path = "/tmp/kek-async.sock";

        let _ = std::fs::remove_file(path);

        let listener = UnixDatagram::bind(path).unwrap();
        let notifier = SdNotify::from_path(path).unwrap();
        notifier.state(Message::ready()).unwrap();
        let mut buf = [0; 100];
        listener.recv(&mut buf).unwrap();
        assert_eq!(&buf[..7], b"READY=1");
    }
}
