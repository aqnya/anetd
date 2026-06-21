#![allow(dead_code)]
pub mod nxdomain;
pub mod proto;
pub mod status;
pub mod wire;

use std::io;
use tokio::io::AsyncWrite;

pub async fn send_dns_hard_block<W: AsyncWrite + Unpin>(w: &mut W) -> io::Result<()> {
    proto::getaddrinfo::send_nxdomain(w).await
}

pub async fn send_dns_operation_failed<W: AsyncWrite + Unpin>(w: &mut W) -> io::Result<()> {
    proto::getaddrinfo::send_operation_failed(w).await
}

pub async fn send_resnsend_block<W: AsyncWrite + Unpin>(
    w: &mut W,
    raw_query: &[u8],
) -> io::Result<()> {
    proto::resnsend::send_block(w, raw_query).await
}
