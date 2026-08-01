use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};

use tokio::net::UnixStream;

pub struct Listener {
    active_processes: HashMap<u32, UnixStream>,
}

impl Listener {
    pub fn new() -> Listener {
        Listener {
            active_processes: HashMap::new(),
        }
    }

    pub async fn send(&self, to: u32, message: &str) -> io::Result<()> {
        let Some(stream) = self.active_processes.get(&to) else {
            return Err(Error::new(
                ErrorKind::NotFound,
                "UnixStream not found. Connection possibly closed.",
            ));
        };

        loop {
            stream.writable().await?;

            match stream.try_write(message.as_bytes()) {
                Ok(_) => {
                    break;
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    continue;
                }
                Err(err) => return Err(err.into()),
            }
        }

        Ok(())
    }
}
