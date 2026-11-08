# roze-net

Promiscuous raw sockets, Scapy-style protocol builders/dissectors, and high-speed port scanner written in pure Roze.

## Features

- **Raw Sockets**: Direct `AF_PACKET` / `BPF` raw socket binding in promiscuous mode
- **Layer 2-4 Protocol Stack**: Ethernet II, ARP, IPv4, ICMP, UDP, TCP
- **Checksum Engine**: Automated RFC 1071 16-bit Internet checksum calculation
- **Scapy-Style Protocol Builders**: Fluent packet construction (`ether()`, `ip()`, `tcp()`, `udp()`, `icmp()`, `arp()`)
- **Deep Packet Dissection**: Protocol header decoding and human-readable packet summarization
- **Port Scanner**: Ultra-fast SYN port and banner scanner with non-blocking concurrency
- **PCAP Support**: PCAP 2.4 capture file reading and writing (Wireshark / tcpdump interop)

## Installation

Add to your `bsr.mfz`:

```toml
[dependencies]
roze-net = "^1.0.0"
```

## Usage

```roze
import "roze-net";

func main() {
    let frame = ether_new("00:11:22:33:44:55", "aa:bb:cc:dd:ee:ff", ethertype_ipv4());
    let ip = ip_new("192.168.1.100", "192.168.1.1", ipproto_tcp());
    let tcp = tcp_new(54321, 80, 1000, 0, tcp_flag_syn(), 65535);

    let packet = packet_craft(frame, ip, tcp, "");
    println("Crafted packet length: " + to_string(list_length(packet)) + " bytes");
}
```
