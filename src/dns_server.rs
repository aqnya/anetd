use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::dns::cache::{DnsCache, parse_query_type};
use crate::dns::nxdomain::make_nxdomain_response;
use crate::dns::wire::parse_dns_query_name;
use crate::handlers::format_pseudo_url;
use crate::network::NetworkMonitor;
use crate::rules::{FilterAction, RuleSet};

/// Maximum DNS message size over UDP (RFC 1035).
const MAX_UDP_DNS_SIZE: usize = 512;

/// Upstream forward timeout (seconds).
const UPSTREAM_TIMEOUT_SECS: u64 = 10;

/// Maximum number of concurrent in-flight DNS queries (UDP + TCP combined).
/// Limits memory and task pressure under excessive query load.
const MAX_CONCURRENT_QUERIES: usize = 256;

/// Run the DNS server, listening on `bind_addr` and forwarding allowed queries
/// to the upstream DNS server at `upstream`.  Responses are cached in
/// `dns_cache` to reduce network traffic and battery consumption.
///
/// Each upstream query creates its own UDP socket (following the DnsResolver
/// pattern in `res_send.cpp::send_dg`), so network changes (WiFi ↔ mobile
/// data) never leave in-flight queries stuck on a stale route.  On network
/// change we only flush the DNS cache — stale CDN IPs must be invalidated.
pub async fn run(
    bind_addr: SocketAddr,
    upstream: SocketAddr,
    rules: &'static ArcSwap<RuleSet>,
    dns_cache: &'static DnsCache,
    net_monitor: NetworkMonitor,
) -> io::Result<()> {
    let udp_socket = Arc::new(UdpSocket::bind(bind_addr).await?);
    info!("DNS server (UDP) listening on {}", bind_addr);

    let tcp_listener = TcpListener::bind(bind_addr).await?;
    info!("DNS server (TCP) listening on {}", bind_addr);

    // Shared semaphore to cap in-flight queries and bound memory/task pressure.
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_QUERIES));

    let udp_handle = {
        let udp = udp_socket.clone();
        let upstream = upstream;
        let dns_cache: &'static DnsCache = dns_cache;
        let sem = Arc::clone(&sem);
        tokio::spawn(async move {
            let mut buf = vec![0u8; MAX_UDP_DNS_SIZE];
            loop {
                match udp.recv_from(&mut buf).await {
                    Ok((n, src)) => {
                        let query = buf[..n].to_vec();
                        let r = rules.load_full();
                        let udp = udp.clone();
                        let cache = dns_cache;
                        let sem = sem.clone();
                        tokio::spawn(async move {
                            let _permit = sem.acquire_owned().await;
                            serve_udp(udp, src, query, cache, upstream, r).await;
                        });
                    }
                    Err(e) => error!("DNS UDP recv error: {e}"),
                }
            }
        })
    };

    let tcp_handle = {
        let dns_cache: &'static DnsCache = dns_cache;
        let sem = Arc::clone(&sem);
        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((stream, src)) => {
                        let r = rules.load_full();
                        let cache = dns_cache;
                        let sem = sem.clone();
                        tokio::spawn(async move {
                            let _permit = sem.acquire_owned().await;
                            serve_tcp(stream, src, cache, upstream, r).await;
                        });
                    }
                    Err(e) => error!("DNS TCP accept error: {e}"),
                }
            }
        })
    };

    // On network change, flush the DNS cache — stale CDN IPs from the
    // previous network must be evicted.  Each upstream query creates its
    // own socket, so there is no shared socket to recreate.
    let net_handle = {
        let cache_for_net: &'static DnsCache = dns_cache;
        tokio::spawn(async move {
            loop {
                net_monitor.notified().await;
                info!("network change — flushing DNS cache");
                cache_for_net.flush().await;
            }
        })
    };

    tokio::select! {
        res = udp_handle => { res?; }
        res = tcp_handle => { res?; }
        res = net_handle => { res?; }
    }

    Ok(())
}

/// Handle a single UDP DNS query.
async fn serve_udp(
    listener: Arc<UdpSocket>,
    src: SocketAddr,
    query: Vec<u8>,
    cache: &'static DnsCache,
    upstream_addr: SocketAddr,
    rules: Arc<RuleSet>,
) {
    if let Err(e) = serve_udp_inner(&listener, src, &query, cache, upstream_addr, &rules).await {
        error!("DNS UDP error from {src}: {e}");
    }
}

async fn serve_udp_inner(
    listener: &UdpSocket,
    src: SocketAddr,
    query: &[u8],
    cache: &'static DnsCache,
    upstream_addr: SocketAddr,
    rules: &RuleSet,
) -> io::Result<()> {
    let hostname = parse_dns_query_name(query);

    match hostname {
        Some(ref name) => {
            let pseudo_url = format_pseudo_url(name);
            let action = rules.matches(&pseudo_url, name, "other");

            match action {
                FilterAction::Block => {
                    let nx = make_nxdomain_response(query).unwrap_or_else(|| query.to_vec());
                    listener.send_to(&nx, src).await?;
                    info!("[DNS BLOCKED] {name}");
                }
                FilterAction::Allow => {
                    let response = forward_or_cache(query, cache, upstream_addr).await?;
                    listener.send_to(&response, src).await?;
                }
            }
        }
        None => {
            let response = forward_upstream(query, upstream_addr).await?;
            listener.send_to(&response, src).await?;
        }
    }

    Ok(())
}

/// Handle a single TCP DNS connection.
async fn serve_tcp(
    mut stream: TcpStream,
    src: SocketAddr,
    cache: &'static DnsCache,
    upstream_addr: SocketAddr,
    rules: Arc<RuleSet>,
) {
    if let Err(e) = serve_tcp_inner(&mut stream, &src, cache, upstream_addr, &rules).await {
        error!("DNS TCP error from {src}: {e}");
    }
}

async fn serve_tcp_inner(
    stream: &mut TcpStream,
    src: &SocketAddr,
    cache: &'static DnsCache,
    upstream_addr: SocketAddr,
    rules: &RuleSet,
) -> io::Result<()> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u16::from_be_bytes(len_buf) as usize;

    if msg_len > MAX_UDP_DNS_SIZE {
        warn!("DNS TCP query too large from {src}: {msg_len} bytes");
        return Ok(());
    }

    let mut query = vec![0u8; msg_len];
    stream.read_exact(&mut query).await?;

    let response = match parse_dns_query_name(&query) {
        Some(ref name) => {
            let pseudo_url = format_pseudo_url(name);
            let action = rules.matches(&pseudo_url, name, "other");

            match action {
                FilterAction::Block => {
                    info!("[DNS BLOCKED] {name}");
                    make_nxdomain_response(&query).unwrap_or(query)
                }
                FilterAction::Allow => match forward_or_cache(&query, cache, upstream_addr).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        error!("DNS TCP upstream error for {name}: {e}");
                        return Err(e);
                    }
                },
            }
        }
        None => match forward_upstream(&query, upstream_addr).await {
            Ok(resp) => resp,
            Err(e) => return Err(e),
        },
    };

    let resp_len = response.len() as u16;
    stream.write_all(&resp_len.to_be_bytes()).await?;
    stream.write_all(&response).await?;

    Ok(())
}

/// Check cache, then forward if miss. Stores result on upstream success.
async fn forward_or_cache(
    query: &[u8],
    cache: &'static DnsCache,
    upstream_addr: SocketAddr,
) -> io::Result<Vec<u8>> {
    let hostname = match parse_dns_query_name(query) {
        Some(n) => n,
        None => return forward_upstream(query, upstream_addr).await,
    };

    let qtype = parse_query_type(query).unwrap_or(1); // default to A

    if let Some(cached) = cache.get(&hostname, qtype).await {
        return Ok(cached);
    }

    let response = forward_upstream(query, upstream_addr).await?;
    cache.put(&hostname, qtype, &response).await;
    Ok(response)
}

/// Forward a DNS query to the upstream server via a fresh, per-query UDP socket.
///
/// Following the DnsResolver pattern (`res_send.cpp::send_dg`), each query
/// creates its own socket.  This means network changes (WiFi ↔ mobile data)
/// don't leave in-flight queries stranded on a stale interface — new queries
/// naturally bind to the current default route.
async fn forward_upstream(query: &[u8], addr: SocketAddr) -> io::Result<Vec<u8>> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(addr).await?;
    sock.send(query).await?;

    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(
        std::time::Duration::from_secs(UPSTREAM_TIMEOUT_SECS),
        sock.recv(&mut buf),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "upstream DNS timeout"))??;

    buf.truncate(n);
    Ok(buf)
}
