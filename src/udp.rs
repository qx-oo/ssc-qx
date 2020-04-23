// use crate::config::ServerAddr;
use crate::utils::get_packet;
// use crate::utils::{tcp_to_udp, udp_to_tcp};
use async_std::sync::Mutex;
use futures::future::try_join;
use std::{collections::HashMap, error::Error, net::SocketAddr, sync::Arc};
use tokio::{
    io,
    io::{AsyncReadExt, ReadHalf, WriteHalf},
    net::{
        tcp,
        udp::{RecvHalf, SendHalf},
        TcpStream, UdpSocket,
    },
    prelude::*,
};

// Udp tunnel
// forward tcp packet to udp and recive udp packet to tcp
pub struct UdpTunnel {
    sock_map: Arc<Mutex<HashMap<String, WriteHalf<TcpStream>>>>,
}

impl<'a> UdpTunnel {
    pub fn new() -> Self {
        Self {
            sock_map: Arc::new(Mutex::new(HashMap::<String, WriteHalf<TcpStream>>::new())),
        }
    }
    // tcp client connection
    // pub async fn tcp_connection(remote_addr: SocketAddr) -> Result<TcpStream, Box<dyn Error>> {
    //     let sock = TcpStream::connect(remote_addr).await?;
    //     Ok(sock)
    // }
    // loop recv data and send
    pub async fn poll(self, host: &SocketAddr) -> Result<(), Box<dyn Error>> {
        let inbound = UdpSocket::bind(&host).await?;
        let (mut r_udp, s_udp) = inbound.split();
        let s_udp = Arc::new(Mutex::new(s_udp));
        loop {
            let _s_udp = s_udp.clone();
            let mut buf = vec![0; 1024];
            let (_, peer) = r_udp.recv_from(&mut buf).await?;

            let (remote, data) = match Self::parse_header(&buf) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("parse packet error: {}", e);
                    continue;
                }
            };

            let peer_addr: String = peer.to_string();
            // let s_id = format!("{}-{}", peer_id, peer_addr);
            let sock_map = self.sock_map.clone();
            println!("recv {}", peer_addr);
            tokio::spawn(async move {
                match Self::server_forward(sock_map, _s_udp.clone(), peer, remote, data).await {
                    Err(e) => eprintln!("Error: forward {}", e),
                    _ => {}
                };
            });
        }
    }

    // parse header, get remote socket
    fn parse_header(data: &Vec<u8>) -> Result<(String, Vec<u8>), Box<dyn Error>> {
        let mut seed = 0;
        let addr_len = data[seed] as usize;
        seed += 1;
        let remote_addr = std::str::from_utf8(&data[seed..addr_len + seed])?;
        seed += addr_len;
        let _data = Vec::from(&data[seed..]);
        Ok((remote_addr.to_owned(), _data))
    }

    // udp forward to tcp
    async fn server_forward(
        sock_map: Arc<Mutex<HashMap<String, WriteHalf<TcpStream>>>>,
        s_udp: Arc<Mutex<SendHalf>>,
        peer: SocketAddr,
        remote_str: String,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn Error>> {
        let s_id: String = peer.to_string();
        let mut map = sock_map.lock().await;
        if !map.contains_key(&s_id) {
            let remote_addr: SocketAddr = remote_str.parse().unwrap();
            let sock = TcpStream::connect(remote_addr).await.unwrap();
            let (mut ri, wi) = io::split(sock);
            map.insert(s_id.clone(), wi);

            // let _udp = s_udp.clone();
            let s_id = s_id.clone();
            let sock_map = sock_map.clone();
            tokio::spawn(async move {
                let mut pocket = vec![0; 1024];
                while let Ok(n) = ri.read(&mut pocket).await {
                    // let pocket = Self::add_server_header(s_id, &buf);
                    {
                        let mut _s_udp = s_udp.lock().await;
                        match _s_udp.send(&pocket).await {
                            Err(e) => eprintln!("Error udp send {}", e),
                            Ok(_) => {}
                        };
                    }
                    if n == 0 {
                        let mut map = sock_map.lock().await;
                        map.remove(&s_id);
                        return;
                    }
                }
            });
        }
        let wi = map.get_mut(&s_id).unwrap();
        wi.write_all(&data).await?;
        Ok(())
    }
}

// pub struct ClientUdpTunnel {
//     proxy_addr: SocketAddr,
//     inbound: TcpStream,
//     remote_addr: String,
// }

// impl ClientUdpTunnel {
//     pub fn new(proxy_addr: SocketAddr, inbound: TcpStream, remote_addr: String) -> Self {
//         Self {
//             proxy_addr: proxy_addr,
//             inbound: inbound,
//             remote_addr: remote_addr,
//         }
//     }
//     // udp client connection
//     // async fn connection(&mut self) -> Result<UdpSocket, Box<dyn Error>> {
//     //     let local_addr: SocketAddr = if self.proxy_addr.is_ipv4() {
//     //         "0.0.0.0:0"
//     //     } else {
//     //         "[::]:0"
//     //     }
//     //     .parse()?;
//     //     Ok(UdpSocket::bind(local_addr).await?)
//     // }
//     fn add_header(remote_addr: String, data: &Vec<u8>) -> Vec<u8> {
//         let remote_addr = remote_addr.as_bytes().to_vec();
//         let len = remote_addr.len() as u8;
//         let mut pocket = vec![len];
//         pocket.extend(remote_addr);
//         pocket.extend(data);
//         pocket
//     }
//     async fn tcp_to_udp(
//         r_tcp: &mut tcp::ReadHalf<'_>,
//         s_udp: &mut SendHalf,
//         remote_addr: String,
//     ) -> Result<(), Box<dyn Error>> {
//         let mut pocket = vec![0; 1024];
//         while let Ok(n) = r_tcp.read(&mut pocket).await {
//             let pocket = Self::add_header(remote_addr.clone(), &pocket);
//             match s_udp.send(&pocket).await {
//                 Err(e) => eprintln!("Error udp send {}", e),
//                 Ok(_) => {}
//             };
//             if n == 0 {
//                 return Ok(());
//             }
//         }
//         Ok(())
//     }
//     async fn udp_to_tcp(
//         r_udp: &mut RecvHalf,
//         w_udp: &mut tcp::WriteHalf<'_>,
//     ) -> Result<(), Box<dyn Error>> {
//         let mut pocket = vec![0; 1024];
//         while let Ok(n) = r_udp.recv(&mut pocket).await {
//             match w_udp.write_all(&pocket).await {
//                 Err(e) => eprintln!("Error udp send {}", e),
//                 Ok(_) => {}
//             };
//             if n == 0 {
//                 return Ok(());
//             }
//         }
//         Ok(())
//     }
// }

fn add_header(remote_addr: String, data: &Vec<u8>) -> Vec<u8> {
    let remote_addr = remote_addr.as_bytes().to_vec();
    let len = remote_addr.len() as u8;
    let mut pocket = vec![len];
    pocket.extend(remote_addr);
    pocket.extend(data);
    pocket
}
async fn tcp_to_udp(
    r_tcp: &mut tcp::ReadHalf<'_>,
    s_udp: &mut SendHalf,
    remote_addr: String,
) -> Result<(), Box<dyn Error>> {
    let mut pocket = vec![0; 1024];
    while let Ok(n) = r_tcp.read(&mut pocket).await {
        let pocket = add_header(remote_addr.clone(), &pocket);
        match s_udp.send(&pocket).await {
            Err(e) => eprintln!("Error udp send {}", e),
            Ok(_) => {}
        };
        if n == 0 {
            return Ok(());
        }
    }
    Ok(())
}
async fn udp_to_tcp(
    r_udp: &mut RecvHalf,
    w_udp: &mut tcp::WriteHalf<'_>,
) -> Result<(), Box<dyn Error>> {
    let mut pocket = vec![0; 1024];
    while let Ok(n) = r_udp.recv(&mut pocket).await {
        match w_udp.write_all(&pocket).await {
            Err(e) => eprintln!("Error udp send {}", e),
            Ok(_) => {}
        };
        if n == 0 {
            return Ok(());
        }
    }
    Ok(())
}

pub async fn udp_transfer(
    proxy_addr: SocketAddr,
    mut inbound: TcpStream,
    remote_addr: String,
) -> Result<(), Box<dyn Error>> {
    let local_addr: SocketAddr = if proxy_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()?;
    let outbound = UdpSocket::bind(local_addr).await?;

    let (mut r_udp, mut s_udp) = outbound.split();
    let (mut r_tcp, mut w_tcp) = inbound.split();
    try_join(
        tcp_to_udp(&mut r_tcp, &mut s_udp, remote_addr.clone()),
        udp_to_tcp(&mut r_udp, &mut w_tcp),
    )
    .await?;
    Ok(())
}

// pub async fn udp_transfer(
//     mut inbound: TcpStream,
//     proxy_addr: SocketAddr,
//     remote_addr: ServerAddr,
// ) -> Result<(), Box<dyn Error>> {
//     let local_addr: SocketAddr = if proxy_addr.is_ipv4() {
//         "0.0.0.0:0"
//     } else {
//         "[::]:0"
//     }
//     .parse()?;
//     let outbound = UdpSocket::bind(local_addr).await?;

//     let pocket = get_packet(&remote_addr);
//     // outbound.send_to(&pocket, proxy_addr).await?;

//     let (mut ri, mut wi) = inbound.split();
//     let (mut ro, mut wo) = outbound.split();

//     try_join(
//         tcp_to_udp(&mut ri, &mut wo, &pocket),
//         udp_to_tcp(&mut ro, &mut wi),
//     )
//     .await?;
//     Ok(())
// }
