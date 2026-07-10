use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Default max entries in the DNS cache.
const DEFAULT_MAX_ENTRIES: usize = 2048;

/// Minimum TTL we honor for cache entries (seconds).
const MIN_CACHE_TTL: u64 = 60;

/// Maximum TTL we honor for cache entries (seconds).
/// Caps excessively long TTLs to bound memory usage.
const MAX_CACHE_TTL: u64 = 3600;

/// Default TTL when we cannot parse TTL from the response.
const FALLBACK_TTL: u64 = 300;

/// Cache key: (domain, qtype).
type CacheKey = (String, u16);

struct CacheEntry {
    response: Vec<u8>,
    expires_at: Instant,
}

/// An in-memory, TTL-aware DNS response cache keyed by (domain, query-type).
///
/// Reduces upstream network requests and socket creation for
/// frequently-resolved domains — a significant battery saver on Android.
pub struct DnsCache {
    entries: RwLock<HashMap<CacheKey, CacheEntry>>,
    max_entries: usize,
}

impl DnsCache {
    /// Create a new cache with the specified maximum number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::with_capacity(max_entries.min(256))),
            max_entries,
        }
    }

    /// Look up a cached response.  `query` is the raw DNS query wire-format
    /// bytes; we extract the domain name from the question section to use as
    /// the lookup key.
    ///
    /// Returns `None` on miss or if the entry has expired.
    pub async fn get(&self, domain: &str, qtype: u16) -> Option<Vec<u8>> {
        let key = (domain.to_ascii_lowercase(), qtype);
        let entries = self.entries.read().await;
        let entry = entries.get(&key)?;
        if entry.expires_at <= Instant::now() {
            return None;
        }
        Some(entry.response.clone())
    }

    /// Drop all cached entries.  Called when the network changes (WiFi ↔
    /// mobile data) so that stale CDN IPs from the previous network don't
    /// cause connection failures or suboptimal routing.
    pub async fn flush(&self) {
        self.entries.write().await.clear();
    }

    /// Store a DNS response, extracting TTL from the wire-format response.
    /// If the response does not contain a parseable TTL a fallback is used.
    pub async fn put(&self, domain: &str, qtype: u16, response: &[u8]) {
        let ttl = parse_min_ttl(response);
        let ttl_secs = ttl.clamp(MIN_CACHE_TTL, MAX_CACHE_TTL);
        let expires_at = Instant::now() + Duration::from_secs(ttl_secs);

        let key = (domain.to_ascii_lowercase(), qtype);
        let mut entries = self.entries.write().await;

        // Evict expired entries if at capacity.
        if entries.len() >= self.max_entries {
            entries.retain(|_, e| e.expires_at > Instant::now());
        }
        // If still at capacity, evict LRU-style (earliest expiry).
        if entries.len() >= self.max_entries
            && let Some(oldest_key) = entries
                .iter()
                .min_by_key(|(_, e)| e.expires_at)
                .map(|(k, _)| k.clone())
        {
            entries.remove(&oldest_key);
        }

        entries.insert(
            key,
            CacheEntry {
                response: response.to_vec(),
                expires_at,
            },
        );
    }
}

impl Default for DnsCache {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_ENTRIES)
    }
}

/// Extract the QTYPE from a raw DNS query packet.
/// Returns `None` if the packet is malformed (too short to contain QTYPE).
pub fn parse_query_type(packet: &[u8]) -> Option<u16> {
    if packet.len() < 14 {
        return None;
    }
    // Skip 12-byte header, then skip QNAME (terminated by 0x00).
    let mut pos = 12;
    loop {
        if pos >= packet.len() {
            return None;
        }
        let len = packet[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        // Compression pointer in question — unusual but handle it.
        if (len & 0xC0) == 0xC0 {
            pos += 2;
            break;
        }
        pos += 1 + len;
        if pos > packet.len() {
            return None;
        }
    }
    if pos + 2 > packet.len() {
        return None;
    }
    Some(u16::from_be_bytes([packet[pos], packet[pos + 1]]))
}

/// Parse the minimum TTL from the answer section of a DNS response.
///
/// DNS response layout (RFC 1035):
///   Header       12 bytes
///   Question     variable
///   Answer       (NAME, TYPE(2), CLASS(2), TTL(4), RDLENGTH(2), RDATA)
///
/// We skip the question section by reading QDCOUNT and scanning past each
/// question (QNAME + 4 bytes for QTYPE/QCLASS), then read answer TTLs.
/// Returns `FALLBACK_TTL` on parse failure.
fn parse_min_ttl(packet: &[u8]) -> u64 {
    if packet.len() < 12 {
        return FALLBACK_TTL;
    }

    let qdcount = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    let ancount = u16::from_be_bytes([packet[6], packet[7]]) as usize;

    if ancount == 0 {
        return FALLBACK_TTL;
    }

    let mut pos: usize = 12;

    // Skip the question section.
    for _ in 0..qdcount {
        match skip_name(packet, pos) {
            Some(new_pos) => pos = new_pos + 4, // +4 for QTYPE + QCLASS
            None => return FALLBACK_TTL,
        }
    }

    // Parse answer TTLs, track the minimum.
    let mut min_ttl = u32::MAX;

    for _ in 0..ancount {
        match skip_name(packet, pos) {
            Some(new_pos) => pos = new_pos,
            None => break,
        }

        if pos + 10 > packet.len() {
            break;
        }

        let ttl = u32::from_be_bytes([
            packet[pos + 4],
            packet[pos + 5],
            packet[pos + 6],
            packet[pos + 7],
        ]);
        let rdlength = u16::from_be_bytes([packet[pos + 8], packet[pos + 9]]) as usize;

        if ttl < min_ttl {
            min_ttl = ttl;
        }

        pos += 10 + rdlength;
        if pos > packet.len() {
            break;
        }
    }

    if min_ttl == u32::MAX {
        FALLBACK_TTL
    } else {
        min_ttl as u64
    }
}

/// Skip a DNS name (possibly compressed) and return the position after it.
fn skip_name(packet: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        if pos >= packet.len() {
            return None;
        }
        let len = packet[pos] as usize;
        if len == 0 {
            return Some(pos + 1);
        }
        if (len & 0xC0) == 0xC0 {
            return Some(pos + 2);
        }
        pos += 1 + len;
        if pos > packet.len() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_hit_and_miss() {
        let cache = DnsCache::new(64);
        assert!(cache.get("example.com", 1).await.is_none());

        cache.put("example.com", 1, b"fake_response").await;
        assert_eq!(
            cache.get("example.com", 1).await,
            Some(b"fake_response".to_vec())
        );
        // Different qtype misses.
        assert!(cache.get("example.com", 28).await.is_none());
    }

    #[tokio::test]
    async fn cache_expiry_before_put() {
        let cache = DnsCache::new(64);
        {
            let mut entries = cache.entries.write().await;
            entries.insert(
                ("stale.com".into(), 1),
                CacheEntry {
                    response: b"old".to_vec(),
                    expires_at: Instant::now() - Duration::from_secs(1),
                },
            );
        }
        assert!(cache.get("stale.com", 1).await.is_none());
    }

    #[test]
    fn parse_ttl_valid() {
        let response = build_test_response(300);
        assert_eq!(parse_min_ttl(&response), 300);
    }

    #[test]
    fn parse_ttl_fallback_short() {
        assert_eq!(parse_min_ttl(&[]), FALLBACK_TTL);
        assert_eq!(parse_min_ttl(&[0; 8]), FALLBACK_TTL);
    }

    #[test]
    fn parse_query_type_valid() {
        let query = build_test_query();
        assert_eq!(parse_query_type(&query), Some(1)); // A record
    }

    fn build_test_query() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0x00, 0x01]); // ID
        buf.extend_from_slice(&[0x01, 0x00]); // Flags (standard query)
        buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT
        buf.extend_from_slice(&[0x00, 0x00]); // ANCOUNT
        buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT
        buf.extend_from_slice(&[0x00, 0x00]); // ARCOUNT
        // Question: "example" "com" \0
        buf.extend_from_slice(&[7, b'e', b'x', b'a', b'm', b'p', b'l', b'e']);
        buf.extend_from_slice(&[3, b'c', b'o', b'm', 0]);
        buf.extend_from_slice(&[0x00, 0x01]); // QTYPE = A
        buf.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN
        buf
    }

    fn build_test_response(ttl: u32) -> Vec<u8> {
        let mut buf = build_test_query();
        // Patch flags to indicate response.
        buf[2] = 0x81;
        buf[3] = 0x80;
        // Set ANCOUNT = 1.
        buf[6] = 0x00;
        buf[7] = 0x01;
        // Answer section.
        buf.extend_from_slice(&[0xC0, 0x0C]); // name pointer
        buf.extend_from_slice(&[0x00, 0x01]); // TYPE = A
        buf.extend_from_slice(&[0x00, 0x01]); // CLASS = IN
        buf.extend_from_slice(&ttl.to_be_bytes()); // TTL
        buf.extend_from_slice(&[0x00, 0x04]); // RDLENGTH
        buf.extend_from_slice(&[127, 0, 0, 1]); // 127.0.0.1
        buf
    }
}
