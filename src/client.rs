mod config;
mod tcp;
mod udp;
mod utils;
#[macro_use]
extern crate failure;
use clap::{App, Arg};
use config::{Config, RemoteConfig};
use futures::stream::StreamExt;
use futures::FutureExt;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::Mutex;
use tcp::transfer;
use tokio;
use tokio::net::{TcpListener, TcpStream};
use udp::udp_transfer;
use utils::*;

fn pick_server(config: Arc<Config>) -> Result<RemoteConfig, failure::Error> {
    if config.server_list().is_empty() {
        bail!("server config error");
    }
    let s_cfg = config.server_list()[0].clone();
    Ok(s_cfg)
}

async fn local_server(config: Arc<Config>) -> Result<(), failure::Error> {
    let mut listener = TcpListener::bind(config.host()).await?;
    let sock_map = Arc::new(Mutex::new(HashMap::<String, TcpStream>::new()));
    let server = async move {
        let mut incoming = listener.incoming();
        // let remote_host = remote.host();
        while let Some(socket_res) = incoming.next().await {
            match socket_res {
                Ok(mut socket) => {
                    let peer_addr = socket.peer_addr().unwrap();
                    let peer_addr: String = peer_addr.to_string();
                    println!("Accepted connection from {:?}", peer_addr);

                    let addr = match establish_socks5_connection(&mut socket).await {
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

                    let _sock_map = sock_map.clone();
                    let mut map = _sock_map.lock().unwrap();
                    map.insert(peer_addr, socket);
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
