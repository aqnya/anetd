#!/usr/bin/env python3
"""
Probe dnsproxyd response format for getaddrinfo / gethostbyname requests.
"""

import socket
import struct
import sys

DNSPROXYD_PATH = "/dev/socket/dnsproxyd"

def recv_be32(sock) -> int:
    data = recv_exact(sock, 4)
    return struct.unpack(">i", data)[0]

def recv_exact(sock, n: int) -> bytes:
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise EOFError("connection closed")
        buf += chunk
    return buf

def recv_len_and_data(sock) -> bytes:
    length = recv_be32(sock)
    if length <= 0:
        return b""
    return recv_exact(sock, length)

def recv_status(sock) -> str:
    status = b""
    while True:
        c = sock.recv(1)
        if not c or c == b"\0":
            break
        status += c
    return status.decode()

def send_getaddrinfo(hostname: str, servname: str = "^",
                     hints_flags: int = 0,
                     hints_family: int = 0,
                     hints_socktype: int = 0,
                     hints_protocol: int = 0,
                     net_id: int = 0):
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(DNSPROXYD_PATH)

    # Space-separated ASCII command, NUL-terminated
    cmd = (f"getaddrinfo {hostname} {servname} "
           f"{hints_flags} {hints_family} {hints_socktype} {hints_protocol} "
           f"{net_id}\0")
    sock.sendall(cmd.encode())

    print(f"\n=== getaddrinfo({hostname!r}) ===")
    print(f"  sent: {cmd!r}")

    status = recv_status(sock)
    print(f"  status: {status!r}")

    if not status.startswith("222"):
        remainder = sock.recv(4096)
        print(f"  non-222, remainder ({len(remainder)}B): {remainder.hex()}")
        sock.close()
        return

    # Read addrinfo linked list
    idx = 0
    while True:
        have_more = recv_be32(sock)
        print(f"  have_more: {have_more}")
        if have_more == 0:
            break

        ai_flags    = recv_be32(sock)
        ai_family   = recv_be32(sock)
        ai_socktype = recv_be32(sock)
        ai_protocol = recv_be32(sock)
        addr        = recv_len_and_data(sock)
        canonname   = recv_len_and_data(sock)

        print(f"  [{idx}] flags={ai_flags} family={ai_family} "
              f"socktype={ai_socktype} proto={ai_protocol}")
        print(f"         addr({len(addr)}B): {addr.hex()}")
        print(f"         canonname({len(canonname)}B): {canonname!r}")
        idx += 1

    sock.close()

def send_gethostbyname(hostname: str, net_id: int = 0):
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(DNSPROXYD_PATH)

    # Format: gethostbyname <netid> <hostname> <af>
    cmd = f"gethostbyname {net_id} {hostname} 0\0"
    sock.sendall(cmd.encode())

    print(f"\n=== gethostbyname({hostname!r}) ===")
    print(f"  sent: {cmd!r}")

    status = recv_status(sock)
    print(f"  status: {status!r}")

    if not status.startswith("222"):
        remainder = sock.recv(4096)
        print(f"  non-222, remainder ({len(remainder)}B): {remainder.hex()}")
        sock.close()
        return

    h_name = recv_len_and_data(sock)
    print(f"  h_name({len(h_name)}B): {h_name!r}")

    # Aliases list, terminated by len==0
    while True:
        alias = recv_len_and_data(sock)
        if not alias:
            break
        print(f"  alias: {alias!r}")

    h_addrtype = recv_be32(sock)
    h_length   = recv_be32(sock)
    print(f"  addrtype={h_addrtype} h_length={h_length}")

    # Address list, terminated by len==0
    idx = 0
    while True:
        addr = recv_len_and_data(sock)
        if not addr:
            break
        print(f"  addr[{idx}]({len(addr)}B): {addr.hex()}")
        idx += 1

    sock.close()

if __name__ == "__main__":
    host = sys.argv[1] if len(sys.argv) > 1 else "example.com"
    send_getaddrinfo(host)
    send_gethostbyname(host)