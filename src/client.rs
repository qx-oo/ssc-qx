mod config;
#[macro_use]
extern crate failure;
use clap::{App, Arg};
use config::{Config, RemoteConfig};
use std::error::Error;
// use failure;
use futures::future::try_join;
use futures::stream::StreamExt;
use futures::FutureExt;
use std::fs::File;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
// use std::time;
use std::sync::Arc;
use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

#[derive(Debug)]
pub enum ServerHost {
    Ip(IpAddr),
    Domain(String),
    None,
}

#[derive(Debug)]
pub struct ServerAddr(pub ServerHost, pub u16);

async fn parse_addr(socket: &mut TcpStream, atyp: u8) -> Result<ServerAddr, failure::Error> {
    let host = match atyp {
        0x01 => {
            // ipv4
            let mut addr = [0u8; 4];
            let n = socket.read(&mut addr).await?;
            if n != 4 {
                bail!("addr parse error");
            }
            let addr = Ipv4Addr::from(addr);
            ServerHost::Ip(IpAddr::V4(addr))
        }
        0x03 => {
            // domain
            let mut domain_len = [0u8];
            let n = socket.read(&mut domain_len).await?;
            if n != 1 {
                bail!("domain parse error");
            }
            let domain_len = domain_len[0] as usize;
            let mut domain = vec![0u8; domain_len];
            let n = socket.read(&mut domain).await?;
            if n == 0 {
                bail!("domain parse error");
            }
            let domain = std::str::from_utf8(&domain)?;
            ServerHost::Domain(domain.to_owned())
        }
        0x04 => {
            // ipv6
            let mut addr = [0u8; 16];
            let n = socket.read(&mut addr).await?;
            if n != 16 {
                bail!("v6 addr parse error")
            }
            let addr = Ipv6Addr::from(addr);
            ServerHost::Ip(IpAddr::V6(addr))
        }
        _ => {
            bail!("protocol error");
        }
    };
    let mut port = [0u8; 2];
    let n = socket.read(&mut port).await?;
    if n != 2 {
        bail!("port parse error");
    }
    let port = u16::from_be_bytes(port);
    Ok(ServerAddr(host, port))
}

async fn transfer(mut inbound: TcpStream, proxy_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(proxy_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = io::copy(&mut ri, &mut wo);
    let server_to_client = io::copy(&mut ro, &mut wi);

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}

fn pick_server(config: Arc<Config>) -> Result<RemoteConfig, failure::Error> {
    if config.server_list().is_empty() {
        bail!("server config error");
    }
    let s_cfg = config.server_list()[0].clone();
    // let timeout = time::Duration::new(5, 0);
    // let stream = TcpStream::connect(s_cfg.host()).await?;
    Ok(s_cfg)
}

async fn establish_connection(socket: &mut TcpStream) -> Result<ServerAddr, failure::Error> {
    let mut buf = [0u8];
    let n = socket.read(&mut buf).await?;
    if (n != 1) || (buf[0] != 0x05) {
        bail!("protocol error");
    }
    let mut method_len = [0u8];
    let n = socket.read(&mut method_len).await?;
    if n != 1 {
        bail!("protocol error");
    }
    let method_len = method_len[0] as usize;
    let mut data = vec![0u8; method_len];
    let n = socket.read(&mut data).await?;
    if n != method_len {
        bail!("protocol data error");
    }
    if data.contains(&0x00) {
        // Ok(socket)
        socket.write_all(&[0x05, 0x00]).await?;
    } else {
        bail!("auth not required");
    }
    let mut buf = [0u8; 4];
    let n = socket.read(&mut buf).await?;
    if n != 4 {
        bail!("protocol data error");
    }
    if (buf[0] == 0x05) && (buf[1] == 0x01) && (buf[2] == 0x00) {
    } else {
        bail!("protocol data error");
    }
    let addr = parse_addr(socket, buf[3]).await?;
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

                    let transfer = transfer(socket, remote_config.host().clone()).map(|r| {
                        if let Err(e) = r {
                            println!("Failed to transfer; error={}", e);
                        }
                    });

                    tokio::spawn(transfer);
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
