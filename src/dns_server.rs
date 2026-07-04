use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tracing::{error, info, trace, warn};

use crate::dns::nxdomain::make_nxdomain_response;
use crate::dns::wire::parse_dns_query_name;
use crate::handlers::format_pseudo_url;
use crate::rules::{FilterAction, RuleSet};

/// Maximum DNS message size over UDP (RFC 1035).
const MAX_UDP_DNS_SIZE: usize = 512;

/// Run the DNS server, listening on `bind_addr` and forwarding allowed queries
/// to the upstream DNS server at `upstream`. Rules are reloaded from `rules`
/// on each query via `load_full()`.
pub async fn run(
    bind_addr: SocketAddr,
    upstream: SocketAddr,
    rules: &'static ArcSwap<RuleSet>,
) -> io::Result<()> {
    let udp_socket = Arc::new(UdpSocket::bind(bind_addr).await?);
    info!("DNS server (UDP) listening on {}", bind_addr);

    let tcp_listener = TcpListener::bind(bind_addr).await?;
    info!("DNS server (TCP) listening on {}", bind_addr);

    let udp_handle = {
        let udp = udp_socket.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; MAX_UDP_DNS_SIZE];
            loop {
                match udp.recv_from(&mut buf).await {
                    Ok((n, src)) => {
                        let query = buf[..n].to_vec();
                        let rules = rules.load_full();
                        let udp = udp.clone();
                        tokio::spawn(serve_udp(udp, src, query, upstream, rules));
                    }
                    Err(e) => error!("DNS UDP recv error: {e}"),
                }
            }
        })
    };

    let tcp_handle = tokio::spawn(async move {
        loop {
            match tcp_listener.accept().await {
                Ok((stream, src)) => {
                    let rules = rules.load_full();
                    tokio::spawn(serve_tcp(stream, src, upstream, rules));
                }
                Err(e) => error!("DNS TCP accept error: {e}"),
            }
        }
    });

    tokio::select! {
        res = udp_handle => { res?; }
        res = tcp_handle => { res?; }
    }

    Ok(())
}

/// Handle a single UDP DNS query.
async fn serve_udp(
    listener: Arc<UdpSocket>,
    src: SocketAddr,
    query: Vec<u8>,
    upstream: SocketAddr,
    rules: Arc<RuleSet>,
) {
    if let Err(e) = serve_udp_inner(&listener, src, &query, upstream, &rules).await {
        error!("DNS UDP error from {src}: {e}");
    }
}

async fn serve_udp_inner(
    listener: &UdpSocket,
    src: SocketAddr,
    query: &[u8],
    upstream: SocketAddr,
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
                    let response = forward_upstream(query, upstream).await?;
                    listener.send_to(&response, src).await?;
                    trace!("[DNS ALLOWED] {name}");
                }
            }
        }
        None => {
            // Malformed query — forward to upstream anyway
            let response = forward_upstream(query, upstream).await?;
            listener.send_to(&response, src).await?;
        }
    }

    Ok(())
}

/// Handle a single TCP DNS connection.
async fn serve_tcp(
    mut stream: TcpStream,
    src: SocketAddr,
    upstream: SocketAddr,
    rules: Arc<RuleSet>,
) {
    if let Err(e) = serve_tcp_inner(&mut stream, &src, upstream, &rules).await {
        error!("DNS TCP error from {src}: {e}");
    }
}

async fn serve_tcp_inner(
    stream: &mut TcpStream,
    src: &SocketAddr,
    upstream: SocketAddr,
    rules: &RuleSet,
) -> io::Result<()> {
    // Read 2-byte length prefix
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
                    forward_upstream(&query, upstream).await?
                }
            }
        }
        None => forward_upstream(&query, upstream).await?,
    };

    // Write 2-byte length prefix + response
    let resp_len = response.len() as u16;
    stream.write_all(&resp_len.to_be_bytes()).await?;
    stream.write_all(&response).await?;

    Ok(())
}

/// Forward a DNS query to the upstream server via UDP and return the response.
async fn forward_upstream(query: &[u8], upstream: SocketAddr) -> io::Result<Vec<u8>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(upstream).await?;
    socket.send(query).await?;

    let mut buf = vec![0u8; 4096];
    let n = socket.recv(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}
