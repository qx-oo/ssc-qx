use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::udp::{RecvHalf, SendHalf};
use tokio::prelude::*;

pub async fn tcp_to_udp<'a>(
    reader_i: &mut ReadHalf<'a>,
    writer: &mut SendHalf,
    pocket: &Vec<u8>,
) -> Result<(), io::Error> {
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
