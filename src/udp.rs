use crate::config::ServerAddr;
use crate::utils::get_packet;
use crate::utils::{tcp_to_udp, udp_to_tcp};
use futures::future::try_join;
use std::{
    collections::HashMap,
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    net::{
        tcp::{ReadHalf, WriteHalf},
        udp::SendHalf,
        TcpStream, UdpSocket,
    },
    prelude::*,
};

// Udp tunnel
// forward tcp packet to udp and recive udp packet to tcp
pub struct UdpTunnel<'a> {
    sock_map: Arc<Mutex<HashMap<String, WriteHalf<'a>>>>,
    inbound: Option<UdpSocket>,
    outbound: Option<UdpSocket>,
}

impl<'a> UdpTunnel<'a> {
    pub fn new(sock_map: Arc<Mutex<HashMap<String, WriteHalf<'a>>>>) -> Self {
        Self {
            sock_map: sock_map,
            inbound: None,
            outbound: None,
        }
    }
    // udp client connection
    pub async fn connection(&mut self, proxy_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let local_addr: SocketAddr = if proxy_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }
        .parse()?;
        self.outbound = Some(UdpSocket::bind(local_addr).await?);
        Ok(())
    }
    // tcp client connection
    pub async fn tcp_connection<'b>(
        remote_addr: SocketAddr,
    ) -> Result<&'b mut TcpStream, Box<dyn Error>> {
        let mut sock = TcpStream::connect(remote_addr).await?;
        Ok(&mut sock)
    }
    // udp server listen
    pub async fn listen(&mut self, host: &SocketAddr) -> Result<(), Box<dyn Error>> {
        self.inbound = Some(UdpSocket::bind(&host).await?);
        Ok(())
    }
    // loop recv data and send
    pub async fn poll(&self) -> Result<(), Box<dyn Error>> {
        let inbound = match self.inbound {
            Some(n) => n,
            None => bail!("server config error"),
        };
        let (mut r_udp, mut s_udp) = inbound.split();
        let s_udp = Arc::new(Mutex::new(s_udp));
        loop {
            let mut buf = vec![0; 10240];
            let (n, peer) = r_udp.recv_from(&mut buf).await?;

            let (peer_id, remote, data) = match self.parse_header(&buf) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("parse packet error: {}", e);
                    continue;
                }
            };

            let peer_addr: String = peer.to_string();
            let s_id = format!("{}-{}", peer_id, peer_addr);
            println!("recv {}", s_id);
            tokio::spawn(async move {
                match Self::udp_to_tcp(s_udp, self.sock_map.clone(), remote, s_id, data).await {
                    Err(e) => eprintln!("Error: forward {}", e),
                    _ => {}
                };
            });
            // let map = self.sock_map.lock().unwrap();
            // match map.get(&s_id) {
            //     Some(sock) => {}
            //     None => {
            //         let remote_addr: SocketAddr = match remote.parse() {
            //             Ok(n) => n,
            //             Err(e) => {
            //                 eprintln!("get remote error: {}", e);
            //                 continue;
            //             }
            //         };
            //         let sock = self.tcp_connection(remote_addr).await?;
            //     }
            // };
            // let n = inbound.recv(&mut buf[..]).await?;
            if n > 0 {
                // writer_i.write_all(&buf).await?;
            }
        }
    }

    fn add_header(&self, s_id: String) {}

    // parse header, get remote socket
    fn parse_header(&self, data: &Vec<u8>) -> Result<(String, String, Vec<u8>), Box<dyn Error>> {
        let mut seed = 0;
        let addr_len = data[0] as usize;
        seed += 1;
        let peer_id = std::str::from_utf8(&data[seed..addr_len + seed])?;
        seed += addr_len;
        let addr_len = data[seed] as usize;
        seed += 1;
        let remote_addr = std::str::from_utf8(&data[seed..addr_len + seed])?;
        seed += addr_len;
        let _data = Vec::from(&data[seed..]);
        Ok((peer_id.to_owned(), remote_addr.to_owned(), _data))
    }

    // udp forward to tcp
    async fn udp_to_tcp(
        s_udp: Arc<Mutex<SendHalf>>,
        sock_map: Arc<Mutex<HashMap<String, WriteHalf<'a>>>>,
        remote_str: String,
        s_id: String,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn Error>> {
        let mut map = sock_map.lock().unwrap();
        if !map.contains_key(&s_id) {
            let remote_addr: SocketAddr = remote_str.parse().unwrap();
            let mut sock = TcpStream::connect(remote_addr).await.unwrap();
            let (mut ri, mut wi) = sock.split();
            // let wi = Arc::new((sock, wi));
            map.insert(s_id.clone(), wi);

            let sock_map = sock_map.clone();
            let s_id = s_id.clone();
            // tokio::spawn(async move {
            //     let mut map = sock_map.lock().unwrap();
            //     let sock = map.get_mut(&s_id).unwrap();
            //     // while let Some(data) = sock.poll_read() {}
            // });
        }
        // let sock = map.get_mut(&s_id).unwrap();
        // sock.write_all(&data).await?;
        Ok(())
    }

    async fn tcp_to_udp() {}
}

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
