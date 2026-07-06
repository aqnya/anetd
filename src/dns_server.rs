use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tracing::{error, info, trace, warn};

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

/// Shared context for DNS request handling.
struct ServerCtx {
    upstream: Arc<Mutex<UdpSocket>>,
    cache: &'static DnsCache,
}

/// Run the DNS server, listening on `bind_addr` and forwarding allowed queries
/// to the upstream DNS server at `upstream`.  Responses are cached in
/// `dns_cache` to reduce network traffic and battery consumption.
///
/// On network change (WiFi ↔ mobile data) the DNS cache is flushed and the
/// upstream socket is recreated so it binds to the new default interface.
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

    // Reusable upstream socket: one connected UDP socket serialised via
    // a mutex eliminates per-query bind(0)/connect overhead.
    let upstream_socket = {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        sock.connect(upstream).await?;
        Arc::new(Mutex::new(sock))
    };

    let ctx = Arc::new(ServerCtx {
        upstream: Arc::clone(&upstream_socket),
        cache: dns_cache,
    });

    let udp_handle = {
        let udp = udp_socket.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; MAX_UDP_DNS_SIZE];
            loop {
                match udp.recv_from(&mut buf).await {
                    Ok((n, src)) => {
                        let query = buf[..n].to_vec();
                        let rules = rules.load_full();
                        let udp = udp.clone();
                        let ctx = ctx.clone();
                        tokio::spawn(serve_udp(udp, src, query, ctx, rules));
                    }
                    Err(e) => error!("DNS UDP recv error: {e}"),
                }
            }
        })
    };

    let tcp_handle = {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((stream, src)) => {
                        let rules = rules.load_full();
                        let ctx = ctx.clone();
                        tokio::spawn(serve_tcp(stream, src, ctx, rules));
                    }
                    Err(e) => error!("DNS TCP accept error: {e}"),
                }
            }
        })
    };

    // ── network change handler ──────────────────────────────────────
    // Flush DNS cache and recreate upstream socket on default-route change.
    let upstream_addr = upstream; // copy for use in the task
    let upstream_for_net = upstream_socket.clone();
    let cache_for_net: &'static DnsCache = dns_cache;
    let net_handle = {
        tokio::spawn(async move {
            loop {
                net_monitor.notified().await;
                info!("network change — flushing DNS cache");
                cache_for_net.flush().await;
                match recreate_upstream(&upstream_for_net, upstream_addr).await {
                    Ok(()) => info!("upstream DNS socket recreated for new network"),
                    Err(e) => error!("failed to recreate upstream socket: {e}"),
                }
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

/// Recreate the upstream UDP socket so it binds to the current default
/// interface.  Called on network change (WiFi ↔ mobile data handover).
async fn recreate_upstream(upstream: &Arc<Mutex<UdpSocket>>, addr: SocketAddr) -> io::Result<()> {
    let new_sock = UdpSocket::bind("0.0.0.0:0").await?;
    new_sock.connect(addr).await?;
    let mut sock = upstream.lock().await;
    *sock = new_sock;
    Ok(())
}

/// Handle a single UDP DNS query.
async fn serve_udp(
    listener: Arc<UdpSocket>,
    src: SocketAddr,
    query: Vec<u8>,
    ctx: Arc<ServerCtx>,
    rules: Arc<RuleSet>,
) {
    if let Err(e) = serve_udp_inner(&listener, src, &query, &ctx, &rules).await {
        error!("DNS UDP error from {src}: {e}");
    }
}

async fn serve_udp_inner(
    listener: &UdpSocket,
    src: SocketAddr,
    query: &[u8],
    ctx: &ServerCtx,
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
                    trace!("  dns query matched block rule");
                }
                FilterAction::Allow => {
                    let response = forward_or_cache(query, ctx).await?;
                    listener.send_to(&response, src).await?;
                    trace!("[DNS ALLOWED] {name}");
                }
            }
        }
        None => {
            let response = forward_upstream(query, ctx).await?;
            listener.send_to(&response, src).await?;
        }
    }

    Ok(())
}

/// Handle a single TCP DNS connection.
async fn serve_tcp(
    mut stream: TcpStream,
    src: SocketAddr,
    ctx: Arc<ServerCtx>,
    rules: Arc<RuleSet>,
) {
    if let Err(e) = serve_tcp_inner(&mut stream, &src, &ctx, &rules).await {
        error!("DNS TCP error from {src}: {e}");
    }
}

async fn serve_tcp_inner(
    stream: &mut TcpStream,
    src: &SocketAddr,
    ctx: &ServerCtx,
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
                FilterAction::Allow => {
                    trace!("[DNS ALLOWED] {name}");
                    match forward_or_cache(&query, ctx).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("DNS TCP upstream error for {name}: {e}");
                            return Err(e);
                        }
                    }
                }
            }
        }
        None => match forward_upstream(&query, ctx).await {
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
async fn forward_or_cache(query: &[u8], ctx: &ServerCtx) -> io::Result<Vec<u8>> {
    let hostname = match parse_dns_query_name(query) {
        Some(n) => n,
        None => return forward_upstream(query, ctx).await,
    };

    let qtype = parse_query_type(query).unwrap_or(1); // default to A

    // Check cache first.
    if let Some(cached) = ctx.cache.get(&hostname, qtype).await {
        trace!("[DNS CACHE HIT] {hostname}");
        return Ok(cached);
    }

    // Cache miss — forward upstream.
    let response = forward_upstream(query, ctx).await?;
    ctx.cache.put(&hostname, qtype, &response).await;
    Ok(response)
}

/// Forward a DNS query via the reusable upstream UDP socket.
async fn forward_upstream(query: &[u8], ctx: &ServerCtx) -> io::Result<Vec<u8>> {
    let sock = ctx.upstream.lock().await;
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
