mod config;
mod udp;
mod utils;
#[macro_use]
extern crate failure;
use crate::udp::UdpTunnel;
use clap::{App, Arg};
use config::ServerConfig;
use futures::future::try_join;
use futures::stream::StreamExt;
use futures::FutureExt;
use std::error::Error;
use std::fs::File;
use std::net::SocketAddr;
use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

async fn transfer(mut inbound: TcpStream, proxy_addr: String) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(proxy_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = io::copy(&mut ri, &mut wo);
    let server_to_client = io::copy(&mut ro, &mut wi);

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}

async fn parse_addr(socket: &mut TcpStream) -> Result<String, failure::Error> {
    let mut data = [0u8];
    if 1 != socket.read(&mut data).await? {
        bail!("type error")
    }

    let len = data[0] as usize;

    let mut addr = vec![0u8; len];
    if 0 == socket.read(&mut addr).await? {
        bail!("ip error");
    }
    let addr = std::str::from_utf8(&addr)?;
    Ok(addr.to_owned())
}

async fn start_server(host: &SocketAddr) -> Result<(), failure::Error> {
    let mut listener = TcpListener::bind(host).await?;
    let server = async move {
        let mut incoming = listener.incoming();
        while let Some(socket_res) = incoming.next().await {
            match socket_res {
                Ok(mut socket) => {
                    let peer_addr = match parse_addr(&mut socket).await {
                        Ok(addr) => addr,
                        Err(e) => {
                            println!("Get peer_addr error: {:?}", e);
                            continue;
                        }
                    };
                    println!("Accepted connection from {:?}", peer_addr);
                    let transfer = transfer(socket, peer_addr).map(|r| {
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
    let matches = App::new("SS-server")
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

    let path = matches.value_of("config").unwrap_or("server.json");
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => panic!("配置文件打开错误"),
    };
    let config = match ServerConfig::from_file(&mut file) {
        Ok(c) => c,
        Err(_) => panic!("配置文件错误"),
    };
    println!("config: {:?}", config);

    if config.udp() {
        let udp_tunnel = UdpTunnel::new();
        match udp_tunnel.poll(&config.host()).await {
            Err(e) => panic!("{}", e),
            Ok(_) => {}
        };
    } else {
        match start_server(&config.host()).await {
            Ok(_) => {}
            Err(e) => panic!("错误: {:?}", e),
        };
    }
}
