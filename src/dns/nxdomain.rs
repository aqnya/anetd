pub fn make_nxdomain_response(query: &[u8]) -> Option<Vec<u8>> {
    if query.len() < 12 {
        return None;
    }

    let mut ans = query[0..12].to_vec();
    ans[2] = 0x80 | (query[2] & 0x01); // QR=1, RD= original 
    // RA=1 RCODE=3 (NXDOMAIN)
    ans[3] = 0x83;
    // ANCOUNT / NSCOUNT / ARCOUNT = 0
    ans[6..12].fill(0);

    // Skip QNAME in question section
    let mut pos = 12usize;
    loop {
        if pos >= query.len() {
            return None;
        }
        let label_len = query[pos] as usize;
        if label_len == 0 {
            pos += 1; // include terminating 0x00
            break;
        }
        // Reject compression pointers (0xC0..), which should not appear here
        if (label_len & 0xC0) != 0 {
            return None;
        }
        pos += 1 + label_len;
    }
    // QTYPE + QCLASS (4 bytes)
    if pos + 4 > query.len() {
        return None;
    }
    ans.extend_from_slice(&query[12..pos + 4]);
    Some(ans)
}
