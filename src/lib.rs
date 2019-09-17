use std::env;
use std::os::unix::net::UnixDatagram;
use std::path::Path;

enum State<'a> {
    Ready,
    Status(&'a str),
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
        self.state(State::Ready)
    }

    /// Passes a single-line status string back to the init system that describes the daemon state.
    pub fn status(&self, status: &str) -> Result<(), std::io::Error> {
        self.state(State::Status(status))
    }

    /// Tells systemd to update the watchdog timestamp.
    /// This is the keep-alive ping that services need to issue in regular
    /// intervals if WatchdogSec= is enabled for it.
    pub fn ping_watchdog(&self) -> Result<(), std::io::Error> {
        self.state(State::Watchdog)
    }

    fn state(&self, state: State<'_>) -> Result<(), std::io::Error> {
        match state {
            State::Ready => self.0.send(b"READY=1")?,
            State::Status(status) => self.0.send(format!("STATUS={}", status).as_bytes())?,
            State::Watchdog => self.0.send(b"WATCHDOG=1")?,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ok() {
        use super::*;

        let path = "/tmp/kek.sock";

        std::fs::remove_file(path).unwrap();
        let listener = UnixDatagram::bind(path).unwrap();
        let notifier = SdNotify::from_path(path).unwrap();
        notifier.state(State::Ready).unwrap();
        let mut buf = [0; 100];
        listener.recv(&mut buf).unwrap();
        assert_eq!(&buf[..7], b"READY=1");
    }
}
