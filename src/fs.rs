use std::path::Path;
use tokio::fs::read;
use tokio::io::{Error, ErrorKind};

pub async fn read_file(p: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
    read(p).await
}

pub async fn _read_text(p: impl AsRef<Path>) -> Result<String, Error> {
    let v = read(p).await?;
    match String::from_utf8(v) {
        Ok(text) => Ok(text),
        Err(_) => Err(Error::new(
            ErrorKind::InvalidData,
            "Unable to parse UTF-8 data.",
        )),
    }
}
