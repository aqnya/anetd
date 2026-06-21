use std::future::Future;
use std::io;
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub trait ProtoWrite: AsyncWrite + Unpin {
    fn write_cmd(&mut self, s: &str) -> impl Future<Output = io::Result<()>> + Send
    where
        Self: Send,
    {
        async move {
            let mut buf = Vec::with_capacity(s.len() + 1);
            buf.extend_from_slice(s.as_bytes());
            buf.push(0);
            self.write_all(&buf).await
        }
    }
}

impl<T: AsyncWrite + Unpin + Send> ProtoWrite for T {}
