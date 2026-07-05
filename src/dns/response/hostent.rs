// Response encoding for gethostbyname / gethostbyaddr.
//
// Client-side parsing (android_gethostbynamefornetcontext proxy path):
//   "222 DnsProxyQueryResult\0"
//   sendhostent():
//     sendLenAndData(strlen(h_name)+1, h_name)     <- NUL-terminated
//     foreach alias: sendLenAndData(len+1, alias)
//     sendLenAndData(0, "")                         <- end of aliases
//     be32(h_addrtype)
//     be32(h_length)                                <- bytes per address (IPv4=4)
//     foreach addr: sendLenAndData(16, addr)         <- always 16 bytes!
//     sendLenAndData(0, "")                         <- end of addr_list

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::io::AsyncWrite;

use crate::dns::status::DnsProxyStatus;
use crate::dns::wire::{write_be32, write_len_and_cstring, write_len_and_data};

/// Sends a synthetic IPv4 hostent response for the given hostname and address.
pub async fn send_fake_ipv4<W: AsyncWrite + Unpin>(
    w: &mut W,
    hostname: &str,
    ip: Ipv4Addr,
) -> io::Result<()> {
    DnsProxyStatus::DnsProxyQueryResult.write(w).await?;

    // h_name
    write_len_and_cstring(w, hostname).await?;

    // h_aliases is empty; write end-of-list marker
    write_be32(w, 0).await?;

    // h_addrtype = AF_INET, h_length = 4
    write_be32(w, libc::AF_INET).await?;
    write_be32(w, 4).await?;

    // h_addr_list[0]: bionic's sendhostent always writes a fixed 16-byte buffer
    let mut addr_buf = [0u8; 16];
    addr_buf[..4].copy_from_slice(&ip.octets());
    write_len_and_data(w, &addr_buf).await?;

    // end of addr_list
    write_be32(w, 0).await?;
    Ok(())
}

/// Sends a synthetic IPv6 hostent response for the given hostname and address.
pub async fn send_fake_ipv6<W: AsyncWrite + Unpin>(
    w: &mut W,
    hostname: &str,
    ip: Ipv6Addr,
) -> io::Result<()> {
    DnsProxyStatus::DnsProxyQueryResult.write(w).await?;

    // h_name
    write_len_and_cstring(w, hostname).await?;

    // h_aliases is empty; write end-of-list marker
    write_be32(w, 0).await?;

    // h_addrtype = AF_INET6, h_length = 16
    write_be32(w, libc::AF_INET6).await?;
    write_be32(w, 16).await?;

    // h_addr_list[0]: IPv6 octets are exactly 16 bytes, matching bionic's fixed buffer size
    write_len_and_data(w, &ip.octets()).await?;

    // end of addr_list
    write_be32(w, 0).await?;
    Ok(())
}

/// Sends a generic operation failure response, matching the gethostbyname failure path.
pub async fn send_operation_failed<W: AsyncWrite + Unpin>(w: &mut W) -> io::Result<()> {
    DnsProxyStatus::DnsProxyOperationFailed.write(w).await
}
