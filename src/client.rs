mod config;
mod utils;
#[macro_use]
extern crate failure;
use clap::{App, Arg};
use config::{Config, RemoteConfig};
use std::error::Error;
use utils::{tcp_to_udp, udp_to_tcp};
// use failure;
use futures::future::try_join;
use futures::stream::StreamExt;
use futures::FutureExt;
use std::fs::File;
use std::net::SocketAddr;
use std::net::{Ipv4Addr, Ipv6Addr};
// use std::time;
use std::sync::Arc;
use tokio;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::prelude::*;

#[derive(Debug)]
pub struct ServerAddr(pub String);

async fn parse_socks5_addr(socket: &mut TcpStream, atyp: u8) -> Result<ServerAddr, failure::Error> {
    let host: String = match atyp {
        0x01 => {
            // ipv4
            let mut addr = [0u8; 4];
            if 4 != socket.read(&mut addr).await? {
                bail!("addr parse error");
            }
            Ipv4Addr::from(addr).to_string()
        }
        0x03 => {
            // domain
            let mut domain_len = [0u8];
            if 1 != socket.read(&mut domain_len).await? {
                bail!("domain parse error");
            }
            let domain_len = domain_len[0] as usize;
            let mut domain = vec![0u8; domain_len];
            if domain_len != socket.read(&mut domain).await? {
                bail!("domain parse error");
            }
            let domain = std::str::from_utf8(&domain)?;
            domain.to_owned()
        }
        0x04 => {
            // ipv6
            let mut addr = [0u8; 16];
            if 16 != socket.read(&mut addr).await? {
                bail!("v6 addr parse error")
            }
            Ipv6Addr::from(addr).to_string()
        }
        _ => {
            bail!("protocol error");
        }
    };
    let mut port = [0u8; 2];
    if 2 != socket.read(&mut port).await? {
        bail!("port parse error");
    };
    let port = u16::from_be_bytes(port);
    let addr = format!("{}:{}", host, port);
    Ok(ServerAddr(addr))
}

fn get_packet(remote_addr: &ServerAddr) -> Vec<u8> {
    let addr = remote_addr.0.as_bytes().to_vec();
    let len = addr.len() as u8;
    let mut pocket = vec![len];
    pocket.extend(addr.iter());
    pocket
}

// async fn remote_establish_connection(
//     socket: &mut TcpStream,
//     remote_addr: ServerAddr,
// ) -> Result<(), failure::Error> {
//     let pocket = get_packet(&remote_addr);
//     socket.write_all(&pocket).await?;
//     Ok(())
// }

async fn transfer(
    mut inbound: TcpStream,
    proxy_addr: SocketAddr,
    remote_addr: ServerAddr,
) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(proxy_addr).await?;

    // remote_establish_connection(&mut outbound, remote_addr).await?;
    let pocket = get_packet(&remote_addr);
    outbound.write_all(&pocket).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = io::copy(&mut ri, &mut wo);
    let server_to_client = io::copy(&mut ro, &mut wi);

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}

async fn udp_transfer(
    mut inbound: TcpStream,
    proxy_addr: SocketAddr,
    remote_addr: ServerAddr,
) -> Result<(), Box<dyn Error>> {
    let local_addr: SocketAddr = if proxy_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()?;
    let outbound = UdpSocket::bind(local_addr).await?;

    let pocket = get_packet(&remote_addr);
    // outbound.send_to(&pocket, proxy_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    try_join(
        tcp_to_udp(&mut ri, &mut wo, &pocket),
        udp_to_tcp(&mut ro, &mut wi),
    )
    .await?;
    Ok(())
}

fn pick_server(config: Arc<Config>) -> Result<RemoteConfig, failure::Error> {
    if config.server_list().is_empty() {
        bail!("server config error");
    }
    let s_cfg = config.server_list()[0].clone();
    Ok(s_cfg)
}

async fn establish_connection(socket: &mut TcpStream) -> Result<ServerAddr, failure::Error> {
    let mut buf = [0u8];
    let n = socket.read(&mut buf).await?;
    if (n != 1) || (buf[0] != 0x05) {
        bail!("protocol error");
    }
    let mut method_len = [0u8];
    if 1 != socket.read(&mut method_len).await? {
        bail!("protocol error");
    }
    let method_len = method_len[0] as usize;
    let mut data = vec![0u8; method_len];
    if method_len != socket.read(&mut data).await? {
        bail!("protocol data error");
    }
    if data.contains(&0x00) {
        // Ok(socket)
        socket.write_all(&[0x05, 0x00]).await?;
    } else {
        bail!("auth not required");
    }
    let mut buf = [0u8; 4];
    if 4 != socket.read(&mut buf).await? {
        bail!("protocol data error");
    }
    if (buf[0] == 0x05) && (buf[1] == 0x01) && (buf[2] == 0x00) {
    } else {
        bail!("protocol data error");
    }
    let addr = parse_socks5_addr(socket, buf[3]).await?;
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        .await?;
    Ok(addr)
}

async fn local_server(config: Arc<Config>) -> Result<(), failure::Error> {
    let mut listener = TcpListener::bind(config.host()).await?;
    let server = async move {
        let mut incoming = listener.incoming();
        // let remote_host = remote.host();
        while let Some(socket_res) = incoming.next().await {
            match socket_res {
                Ok(mut socket) => {
                    println!("Accepted connection from {:?}", socket.peer_addr());
                    let addr = match establish_connection(&mut socket).await {
                        Ok(addr) => addr,
                        Err(err) => {
                            println!("establish error {:?}", err);
                            continue;
                        }
                    };
                    println!("addr: {:?}", addr);
                    let remote_config = match pick_server(config.clone()) {
                        Ok(ret) => ret,
                        Err(err) => {
                            println!("establish error {:?}", err);
                            continue;
                        }
                    };

                    if config.udp() {
                        let transfer = udp_transfer(socket, remote_config.host().clone(), addr)
                            .map(|r| {
                                if let Err(e) = r {
                                    println!("Failed to transfer; error={}", e);
                                }
                            });

                        tokio::spawn(transfer);
                    } else {
                        let transfer =
                            transfer(socket, remote_config.host().clone(), addr).map(|r| {
                                if let Err(e) = r {
                                    println!("Failed to transfer; error={}", e);
                                }
                            });

                        tokio::spawn(transfer);
                    }
                }
                Err(err) => {
                    println!("accept error = {:?}", err);
                }
            }
        }
    };
    server.await;
    Ok(())
}

#[tokio::main]
async fn main() {
    let matches = App::new("SS-client")
        .version("1.0")
        .author("qxoo")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .help("add config file")
                .takes_value(true),
        )
        .get_matches();

    let path = matches.value_of("config").unwrap_or("client.json");
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => panic!("配置文件打开错误"),
    };
    let config = match Config::from_file(&mut file) {
        Ok(c) => c,
        Err(_) => panic!("配置文件错误"),
    };
    let config = Arc::new(config);
    println!("config: {:?}", config);
    match local_server(config.clone()).await {
        Ok(_) => (),
        Err(e) => panic!("local server: {}", e),
    };
    println!("Hello, world!");
}
