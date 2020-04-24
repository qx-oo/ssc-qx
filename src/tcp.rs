use crate::config::ServerAddr;
use crate::utils::get_packet;
use futures::future::try_join;
use std::error::Error;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::prelude::*;

pub async fn transfer(
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
