use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use tokio::io::{AsyncReadExt, AsyncWriteExt};


struct HostInfo {
    local_ipv4: Option<Ipv4Addr>,
    public_ipv4: Option<Ipv4Addr>,
    local_ipv6: Option<Ipv6Addr>,
    public_ipv6: Option<Ipv6Addr>,
}

const TIMEOUT_SECS: u64 = 2;

async fn get_host_info() -> HostInfo {
    let (local_v4, public_v4, local_v6, public_v6) = tokio::join!(
        timeout(Duration::from_secs(TIMEOUT_SECS), get_local_ipv4()),
        timeout(Duration::from_secs(TIMEOUT_SECS), public_ip::addr_v4()),
        timeout(Duration::from_secs(TIMEOUT_SECS), get_local_ipv6()),
        timeout(Duration::from_secs(TIMEOUT_SECS), public_ip::addr_v6())
    );

    HostInfo {
        local_ipv4: local_v4.ok().flatten(),
        public_ipv4: public_v4.ok().flatten(),
        local_ipv6: local_v6.ok().flatten(),
        public_ipv6: public_v6.ok().flatten(),
    }
}

async fn get_local_ipv4() -> Option<Ipv4Addr> {
    use std::net::IpAddr;

    tokio::task::spawn_blocking(|| {
        local_ip_address::local_ip().ok().and_then(|ip| match ip {
            IpAddr::V4(ipv4) => Some(ipv4),
            _ => None,
        })
    })
    .await
    .ok()
    .flatten()
}

async fn get_local_ipv6() -> Option<Ipv6Addr> {
    use std::net::IpAddr;

    tokio::task::spawn_blocking(|| {
        local_ip_address::local_ipv6().ok().and_then(|ip| match ip {
            IpAddr::V6(ipv6) => Some(ipv6),
            _ => None,
        })
    })
    .await
    .ok()
    .flatten()
}

async fn find_available_port_parallel(start: u16, end: u16) -> Option<u16> {
    let tasks: Vec<_> = (start..=end)
        .map(|port| tokio::spawn(async move { (port, is_port_available(port).await) }))
        .collect();

    for task in tasks {
        if let Ok((port, available)) = task.await {
            if available {
                return Some(port);
            }
        }
    }

    None
}

async fn is_port_available(port: u16) -> bool {
    let (ipv4_ok, ipv6_ok) = tokio::join!(check_port_ipv4(port), check_port_ipv6(port));

    ipv4_ok && ipv6_ok
}

async fn check_port_ipv4(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
        .await
        .is_ok()
}

async fn check_port_ipv6(port: u16) -> bool {
    TcpListener::bind(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0))
        .await
        .is_ok()
}
async fn handle_client(mut socket: tokio::net::TcpStream, addr: std::net::SocketAddr) {
    println!("New connection from: {}", addr);

    let mut buffer = [0; 1024];

    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                println!("Connection closed by: {}", addr);
                break;
            }
            Ok(n) => {
                println!("Received {} bytes from {}", n, addr);

                // Echo back
                if let Err(e) = socket.write_all(&buffer[..n]).await {
                    eprintln!("Failed to write to {}: {}", addr, e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error reading from {}: {}", addr, e);
                break;
            }
        }
    }
}

async fn run_server_ipv4(listener: TcpListener) {
    println!(
        "IPv4 server listening on {}",
        listener.local_addr().unwrap()
    );

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                tokio::spawn(async move {
                    handle_client(socket, addr).await;
                });
            }
            Err(e) => {
                eprintln!("IPv4 accept error: {}", e);
            }
        }
    }
}

async fn run_server_ipv6(listener: TcpListener) {
    println!(
        "IPv6 server listening on {}",
        listener.local_addr().unwrap()
    );

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                tokio::spawn(async move {
                    handle_client(socket, addr).await;
                });
            }
            Err(e) => {
                eprintln!("IPv6 accept error: {}", e);
            }
        }
    }
}
#[tokio::main]
async fn main() {
    let info = get_host_info().await;

    match info.local_ipv4 {
        Some(ip) => println!("Local IPv4: {}", ip),
        None => eprintln!("Failed to get local IPv4"),
    }

    match info.public_ipv4 {
        Some(ip) => println!("Public IPv4: {}", ip),
        None => eprintln!("Failed to get public IPv4"),
    }

    match info.local_ipv6 {
        Some(ip) => println!("Local IPv6: {}", ip),
        None => eprintln!("Failed to get local IPv6"),
    }

    match info.public_ipv6 {
        Some(ip) => println!("Public IPv6: {}", ip),
        None => eprintln!("Failed to get public IPv6"),
    }

    match find_available_port_parallel(6881, 6900).await {
        Some(port) => {
            println!("Found available port: {}", port);

            let ipv4_listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
                .await
                .unwrap();

            let ipv6_listener =
                TcpListener::bind(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0))
                    .await
                    .unwrap();

            println!("Servers started on port {}", port);

            tokio::join!(
                run_server_ipv4(ipv4_listener),
                run_server_ipv6(ipv6_listener)
            );
        }
        None => eprintln!("No available port found in range 6882-6900"),
    }
}
