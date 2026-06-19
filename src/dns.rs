// src/dns.rs
#![allow(dead_code)]

use std::io::{self, Write};
use std::net::Ipv4Addr;

/// 写入 4 字节大端序整型 (对应 C++ 里的 sendBE32)
pub fn write_be32(w: &mut impl Write, value: i32) -> io::Result<()> {
    w.write_all(&value.to_be_bytes())
}

/// 先写入 4 字节大端长度，再写入二进制数据流 (对应 C++ 里的 sendLenAndData)
pub fn write_len_and_data(w: &mut impl Write, data: &[u8]) -> io::Result<()> {
    // 修复：安全转换长度，溢出时安全降级为 0
    let len = i32::try_from(data.len()).unwrap_or(0);
    write_be32(w, len)?;
    if len > 0 {
        w.write_all(data)?;
    }
    Ok(())
}

/// 写入带 C 风格字符串结尾(\0)的文本，并自动计算包含空字符在内的总长度
pub fn write_len_and_string(w: &mut impl Write, s: &str) -> io::Result<()> {
    if s.is_empty() {
        write_be32(w, 0)?;
    } else {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0); // 补齐 C-String 的 \0
        write_len_and_data(w, &bytes)?;
    }
    Ok(())
}

/// 统一返回通用操作失败状态码 (对应原生的 sendBinaryMsg(ResponseCode::DnsProxyOperationFailed, ...))
pub fn send_dns_operation_failed(w: &mut impl Write) -> io::Result<()> {
    w.write_all(b"223 DnsProxyOperationFailed\0")?;
    Ok(())
}

/// 统一硬阻断响应：利用原生 netd 的错误处理机制直接告知系统该域名无数据/已拦截
pub fn send_dns_hard_block(w: &mut impl Write) -> io::Result<()> {
    w.write_all(b"222 DnsProxyQueryResult\0")?;
    write_be32(w, 1)?; // 状态码: 1 (代表有错/无数据)
    write_be32(w, 0)?; // 后续载荷包长度: 0
    Ok(())
}

/// 伪造 `getaddrinfo` 的单一 IPv4 响应包
pub fn send_addrinfo_fake_response(w: &mut impl Write, ip: Ipv4Addr) -> io::Result<()> {
    w.write_all(b"222 DnsProxyQueryResult\0")?;

    write_be32(w, 0)?; // ai_flags
    write_be32(w, 2)?; // ai_family (AF_INET = 2)
    write_be32(w, 1)?; // ai_socktype (SOCK_STREAM = 1)
    write_be32(w, 6)?; // ai_protocol (IPPROTO_TCP = 6)

    let mut sockaddr = Vec::with_capacity(16);
    sockaddr.extend_from_slice(&2u16.to_ne_bytes()); // sin_family
    sockaddr.extend_from_slice(&0u16.to_be_bytes()); // sin_port (0)
    sockaddr.extend_from_slice(&ip.octets()); // sin_addr (4 字节 IPv4)
    sockaddr.resize(16, 0); // padding 补齐到 16 字节

    write_len_and_data(w, &sockaddr)?;
    write_be32(w, 0)?; // ai_canonname 别名
    write_be32(w, 0)?; // 标识链表结束

    Ok(())
}

/// 伪造 `gethostbyname` / `gethostbyaddr` 的主控响应包
pub fn send_hostent_fake_response(
    w: &mut impl Write,
    hostname: &str,
    ip: Ipv4Addr,
) -> io::Result<()> {
    w.write_all(b"222 DnsProxyQueryResult\0")?;

    write_len_and_string(w, hostname)?;
    write_be32(w, 0)?; // hp->h_aliases
    write_be32(w, 2)?; // hp->h_addrtype (AF_INET = 2)
    write_be32(w, 4)?; // hp->h_length (IPv4 = 4字节)

    write_len_and_data(w, &ip.octets())?;
    write_be32(w, 0)?; // 标识 IP 数组结束

    Ok(())
}

/// 统一封装 resnsend 的裸 DNS 数据包响应
pub fn send_resnsend_raw_packet(w: &mut impl Write, dns_packet: &[u8]) -> io::Result<()> {
    w.write_all(b"222 DnsProxyQueryResult\0")?;
    write_be32(w, 0)?; // 状态码：0
    write_be32(w, i32::try_from(dns_packet.len()).unwrap_or(0))?;

    w.write_all(dns_packet)?;
    Ok(())
}
