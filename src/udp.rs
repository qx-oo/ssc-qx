use crate::config::ServerAddr;
use crate::utils::get_packet;
use crate::utils::{tcp_to_udp, udp_to_tcp};
use futures::future::try_join;
use std::error::Error;
use std::net::SocketAddr;
use tokio::net::{TcpStream, UdpSocket};

pub async fn udp_transfer(
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
