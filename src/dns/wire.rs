use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Maximum DNS payload size (RFC 1035 limits UDP DNS to 512 bytes,
/// but TCP DNS and extended responses can be up to 65535 bytes).
const MAX_DNS_PAYLOAD: usize = 65535;

/// Writes a signed 32-bit integer in big-endian byte order.
/// This is the fundamental unit of the dnsproxyd wire protocol.
pub async fn write_be32(
    w: &mut (impl tokio::io::AsyncWrite + Unpin),
    value: i32,
) -> io::Result<()> {
    w.write_all(&value.to_be_bytes()).await
}

/// Writes a length-prefixed byte buffer: be32(len) followed by the data bytes.
/// If len == 0, only the length field is written (no data body follows).
/// Mirrors AOSP's DnsProxyListener::sendLenAndData().
pub async fn write_len_and_data(
    w: &mut (impl tokio::io::AsyncWrite + Unpin),
    data: &[u8],
) -> io::Result<()> {
    let len = i32::try_from(data.len()).unwrap_or(0);
    write_be32(w, len).await?;
    if len > 0 {
        w.write_all(data).await?;
    }
    Ok(())
}

/// Writes a NUL-terminated C string as a length-prefixed field:
/// be32(strlen + 1) + bytes + NUL.
/// An empty string writes be32(0) with no data body,
/// matching sendLenAndData(0, NULL) on the C side.
pub async fn write_len_and_cstring(
    w: &mut (impl tokio::io::AsyncWrite + Unpin),
    s: &str,
) -> io::Result<()> {
    if s.is_empty() {
        write_be32(w, 0).await
    } else {
        let bytes = s.as_bytes();
        let len = i32::try_from(bytes.len() + 1).unwrap_or(0);
        write_be32(w, len).await?;
        w.write_all(bytes).await?;
        w.write_all(&[0u8]).await
    }
}

/// Reads a signed 32-bit integer in big-endian byte order.
pub async fn read_be32(r: &mut (impl tokio::io::AsyncRead + Unpin)) -> io::Result<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf).await?;
    Ok(i32::from_be_bytes(buf))
}

/// Reads a length-prefixed byte buffer: reads be32(len), then reads exactly len bytes.
/// Returns an empty Vec if len <= 0 without reading further.
/// Returns an error if len exceeds MAX_DNS_PAYLOAD to prevent OOM from malformed input.
pub async fn read_len_and_data(r: &mut (impl tokio::io::AsyncRead + Unpin)) -> io::Result<Vec<u8>> {
    let len = read_be32(r).await?;
    if len <= 0 {
        return Ok(Vec::new());
    }
    let len = len as usize;
    if len > MAX_DNS_PAYLOAD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload too large",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}