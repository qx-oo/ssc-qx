use crate::config::ServerAddr;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::udp::{RecvHalf, SendHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;

pub fn get_packet(remote_addr: &ServerAddr) -> Vec<u8> {
    let addr = remote_addr.0.as_bytes().to_vec();
    let len = addr.len() as u8;
    let mut pocket = vec![len];
    pocket.extend(addr.iter());
    pocket
}

pub async fn tcp_to_udp<'a>(
    reader_i: &mut ReadHalf<'a>,
    writer: &mut SendHalf,
    pocket: &Vec<u8>,
) -> Result<(), io::Error> {
    // convert tcp packet to udp
    loop {
        let mut buf = vec![0; 1024];
        let n = reader_i.read(&mut buf).await?;
        if n > 0 {
            let mut send_pocket = pocket.clone();
            send_pocket.append(&mut buf);
            writer.send(&send_pocket[..]).await?;
        }
    }
}

pub async fn udp_to_tcp<'a>(
    reader: &mut RecvHalf,
    writer_i: &mut WriteHalf<'a>,
) -> Result<(), io::Error> {
    loop {
        let mut buf = vec![0; 1024];
        let n = reader.recv(&mut buf[..]).await?;

        if n > 0 {
            writer_i.write_all(&buf).await?;
        }
    }
}

pub async fn parse_socks5_addr(
    socket: &mut TcpStream,
    atyp: u8,
) -> Result<ServerAddr, failure::Error> {
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

pub async fn establish_socks5_connection(
    socket: &mut TcpStream,
) -> Result<ServerAddr, failure::Error> {
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
