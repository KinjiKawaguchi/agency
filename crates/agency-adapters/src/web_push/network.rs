//! Where a push message may be sent. The host is checked against a list when the subscription is made and
//! again when a message is sent; this checks the addresses the name resolves to when connecting, so that a
//! name that later points inside the machine's network (DNS rebinding) is refused too.
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Whether an address is one on the public internet: not this machine, not a private, shared, link-local,
/// documentation, multicast, or reserved range.
pub fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    let is_shared = a == 100 && (64..128).contains(&b);
    let is_benchmark = a == 198 && (18..20).contains(&b);
    let is_protocol = a == 192 && b == 0 && c == 0;
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_multicast()
        || a == 0
        || a >= 240
        || is_shared
        || is_benchmark
        || is_protocol)
}

/// Only global unicast (`2000::/3`), without the documentation range and the 6to4 range that embeds an
/// IPv4 address. An IPv4-mapped address is not in `2000::/3`, so it is refused.
fn is_public_v6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    let is_global = segments[0] & 0xe000 == 0x2000;
    let is_documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;
    let is_6to4 = segments[0] == 0x2002;
    is_global && !is_documentation && !is_6to4
}

/// Resolves a name with the system resolver and keeps only its public addresses; a name with none fails.
pub struct PublicOnly;

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let found = tokio::net::lookup_host((name.as_str(), 0)).await?;
            let public: Vec<SocketAddr> = found.filter(|one| is_public(one.ip())).collect();
            if public.is_empty() {
                return Err("the name does not resolve to a public address".into());
            }
            let addresses: Addrs = Box::new(public.into_iter());
            Ok(addresses)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public(text: &str) -> bool {
        is_public(text.parse().unwrap())
    }

    #[test]
    fn only_addresses_on_the_public_internet_are_public() {
        for refused in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "100.127.255.255",
            "198.18.0.1",
            "192.0.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "192.0.2.1",
            "::1",
            "::",
            "fe80::1",
            "fc00::1",
            "fd12::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "2001:db8::1",
            "2002:7f00:1::1",
            "ff02::1",
        ] {
            assert!(!public(refused), "{refused} is not public");
        }
        for accepted in [
            "8.8.8.8",
            "142.250.1.1",
            "172.32.0.1",
            "100.128.0.1",
            "2a00:1450::1",
            "2606:4700::1",
        ] {
            assert!(public(accepted), "{accepted} is public");
        }
    }

    #[tokio::test]
    async fn a_name_that_resolves_only_inside_the_machine_is_refused() {
        let outcome = PublicOnly.resolve("localhost".parse().unwrap()).await;
        assert!(outcome.is_err());
    }
}
