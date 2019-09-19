use std::env;
use std::path::{Path, PathBuf};

use futures::sink::Sink;
use futures::{AsyncSink, Poll, StartSend};
use tokio_codec::Encoder;
use tokio_uds::{UnixDatagram, UnixDatagramFramed};

use crate::{Error, InnerMessage, Message};
use bytes::BytesMut;

struct Codec;

impl Encoder for Codec {
    type Item = Message;
    type Error = std::io::Error;

    fn encode(&mut self, item: Self::Item, bytes: &mut BytesMut) -> Result<(), Self::Error> {
        match item.0 {
            InnerMessage::Ready => bytes.extend_from_slice(b"READY=1"),
            InnerMessage::Status(status) => {
                bytes.extend_from_slice(format!("STATUS={}", status).as_bytes())
            }
            InnerMessage::Watchdog => bytes.extend_from_slice(b"WATCHDOG=1"),
        }
        Ok(())
    }
}

pub struct SdNotify {
    path: PathBuf,
    framed: UnixDatagramFramed<PathBuf, Codec>,
}

impl Sink for SdNotify {
    type SinkItem = Message;
    type SinkError = std::io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.framed.start_send((item, self.path.clone()))? {
            AsyncSink::NotReady((item, _)) => Ok(AsyncSink::NotReady(item)),
            AsyncSink::Ready => Ok(AsyncSink::Ready),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.framed.poll_complete()
    }
}

impl SdNotify {
    pub fn from_env() -> Result<Self, Error> {
        let sockname = env::var("NOTIFY_SOCKET")?;
        Self::from_path(sockname)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let socket = UnixDatagram::unbound()?;
        socket.connect(path.as_ref())?;
        Ok(SdNotify {
            framed: UnixDatagramFramed::new(socket, Codec),
            path: path.as_ref().to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ok() {
        use super::SdNotify;
        use crate::Message;
        use std::os::unix::net::UnixDatagram;
        use tokio::prelude::*;
        use tokio::runtime::current_thread::Runtime;

        let path = "/tmp/kek.sock";

        let _ = std::fs::remove_file(path);

        let listener = UnixDatagram::bind(path).unwrap();
        let notifier = SdNotify::from_path(path).unwrap();
        let mut rt = Runtime::new().unwrap();
        rt.block_on(notifier.send(Message::ready())).unwrap();
        let mut buf = [0; 100];
        listener.recv(&mut buf).unwrap();
        assert_eq!(&buf[..7], b"READY=1");
    }
}
