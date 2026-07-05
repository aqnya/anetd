// Response encoding for GetAddrInfoHandler.
//
// Client-side parsing (getaddrinfo.c android_getaddrinfo_proxy):
//   "222 DnsProxyQueryResult\0"
//   loop:
//     readBE32 → have_more (0 = end of list)
//     readBE32 → ai_flags
//     readBE32 → ai_family
//     readBE32 → ai_socktype
//     readBE32 → ai_protocol
//     readBE32 → addr_len  (= sizeof sockaddr_in or sockaddr_in6)
//     read(addr_len) → ai_addr
//     readBE32 → name_len
//     read(name_len) → ai_canonname  (NUL-terminated; 0 = NULL)
//   sendBE32(0) → end of linked list

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::io::AsyncWrite;

use crate::dns::status::DnsProxyStatus;
use crate::dns::wire::{write_be32, write_len_and_cstring, write_len_and_data};

/// Builds a sockaddr_in (16 bytes, matching the C struct layout).
fn sockaddr_in(ip: Ipv4Addr, port: u16) -> [u8; 16] {
    let mut s = [0u8; 16];
    s[0..2].copy_from_slice(&(libc::AF_INET as u16).to_ne_bytes()); // sin_family
    s[2..4].copy_from_slice(&port.to_be_bytes()); // sin_port
    s[4..8].copy_from_slice(&ip.octets()); // sin_addr
    // [8..16] padding = 0
    s
}

/// Builds a sockaddr_in6 (28 bytes, matching the C struct layout).
fn sockaddr_in6(ip: Ipv6Addr, port: u16) -> [u8; 28] {
    let mut s = [0u8; 28];
    s[0..2].copy_from_slice(&(libc::AF_INET6 as u16).to_ne_bytes()); // sin6_family
    s[2..4].copy_from_slice(&port.to_be_bytes()); // sin6_port
    // sin6_flowinfo [4..8] = 0
    s[8..24].copy_from_slice(&ip.octets()); // sin6_addr
    // sin6_scope_id [24..28] = 0
    s
}

/// Writes a single addrinfo entry in the dnsproxyd wire format.
async fn write_addrinfo_entry<W: AsyncWrite + Unpin>(
    w: &mut W,
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    addr: &[u8],
    canonname: Option<&str>,
) -> io::Result<()> {
    write_be32(w, 1).await?; // have_more = 1
    write_be32(w, ai_flags).await?;
    write_be32(w, ai_family).await?;
    write_be32(w, ai_socktype).await?;
    write_be32(w, ai_protocol).await?;
    write_len_and_data(w, addr).await?;
    match canonname {
        Some(name) => write_len_and_cstring(w, name).await?,
        None => write_be32(w, 0).await?, // sendLenAndData(0, NULL)
    }
    Ok(())
}

/// Sends a synthetic AF_INET / SOCK_STREAM addrinfo response for the given IPv4 address.
pub async fn send_fake_ipv4<W: AsyncWrite + Unpin>(w: &mut W, ip: Ipv4Addr) -> io::Result<()> {
    DnsProxyStatus::DnsProxyQueryResult.write(w).await?;
    let addr = sockaddr_in(ip, 0);
    write_addrinfo_entry(
        w,
        0,                 // ai_flags
        libc::AF_INET,     // 2
        libc::SOCK_STREAM, // 1
        libc::IPPROTO_TCP, // 6
        &addr,
        None,
    )
    .await?;
    write_be32(w, 0).await?; // end of linked list
    Ok(())
}

/// Sends a synthetic AF_INET6 / SOCK_STREAM addrinfo response for the given IPv6 address.
pub async fn send_fake_ipv6<W: AsyncWrite + Unpin>(w: &mut W, ip: Ipv6Addr) -> io::Result<()> {
    DnsProxyStatus::DnsProxyQueryResult.write(w).await?;
    let addr = sockaddr_in6(ip, 0);
    write_addrinfo_entry(
        w,
        0,
        libc::AF_INET6,
        libc::SOCK_STREAM,
        libc::IPPROTO_TCP,
        &addr,
        None,
    )
    .await?;
    write_be32(w, 0).await?; // end of linked list
    Ok(())
}

/// Sends a domain-not-found failure response (EAI_NONAME),
/// matching what netd returns for NXDOMAIN queries.
pub async fn send_nxdomain<W: AsyncWrite + Unpin>(w: &mut W) -> io::Result<()> {
    DnsProxyStatus::DnsProxyOperationFailed.write(w).await?;
    write_be32(w, libc::EAI_NONAME).await
}

/// Sends a generic operation failure response (EAI_SYSTEM / ECONNREFUSED).
pub async fn send_operation_failed<W: AsyncWrite + Unpin>(w: &mut W) -> io::Result<()> {
    DnsProxyStatus::DnsProxyOperationFailed.write(w).await?;
    write_be32(w, libc::EAI_SYSTEM).await
}
