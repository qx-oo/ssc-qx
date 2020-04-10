mod config;
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

async fn transfer(mut inbound: TcpStream, proxy_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(proxy_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = io::copy(&mut ri, &mut wo);
    let server_to_client = io::copy(&mut ro, &mut wi);

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}

async fn start_server(host: &SocketAddr) -> Result<(), failure::Error> {
    let mut listener = TcpListener::bind(host).await?;
    let server = async move {
        let mut incoming = listener.incoming();
        while let Some(socket_res) = incoming.next().await {
            match socket_res {
                Ok(mut socket) => {
                    let peer_addr = match socket.peer_addr() {
                        Ok(n) => n,
                        Err(_) => {
                            println!("Get peer_addr error");
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
                    // let addr = match establish_connection(&mut socket).await {
                    //     Ok(addr) => addr,
                    //     Err(err) => {
                    //         println!("establish error {:?}", err);
                    //         continue;
                    //     }
                    // };
                    println!("addr");
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

    match start_server(&config.host()).await {
        Ok(_) => {}
        Err(e) => panic!("错误: {:?}", e),
    };
}
