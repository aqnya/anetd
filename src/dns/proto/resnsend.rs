use std::io;
use tokio::io::AsyncWrite;

use crate::dns::nxdomain::make_nxdomain_response;
use crate::dns::status::DnsProxyStatus;
use crate::dns::wire::{write_be32, write_len_and_data};

/// Sends a raw DNS packet response.
/// rcode > 0: DNS RCODE value (e.g. 3 = NXDOMAIN).
/// rcode < 0: negated errno (failure path, e.g. -ECONNREFUSED).
pub async fn send_raw_packet<W: AsyncWrite + Unpin>(
    w: &mut W,
    rcode: i32,
    dns_packet: &[u8],
) -> io::Result<()> {
    DnsProxyStatus::DnsProxyQueryResult.write(w).await?;
    write_be32(w, rcode).await?;
    write_len_and_data(w, dns_packet).await
}

/// Sends a failure response with a negated errno and no packet body.
pub async fn send_errno<W: AsyncWrite + Unpin>(w: &mut W, neg_errno: i32) -> io::Result<()> {
    DnsProxyStatus::DnsProxyQueryResult.write(w).await?;
    write_be32(w, neg_errno).await
}

/// Sends a block response for a DNS query.
/// Attempts to construct a well-formed NXDOMAIN reply from the original query;
/// falls back to ECONNREFUSED if the query is malformed or unparseable.
pub async fn send_block<W: AsyncWrite + Unpin>(w: &mut W, raw_query: &[u8]) -> io::Result<()> {
    match make_nxdomain_response(raw_query) {
        Some(nx) => send_raw_packet(w, 3, &nx).await, // RCODE 3 = NXDOMAIN
        None => send_errno(w, -libc::ECONNREFUSED).await,
    }
}