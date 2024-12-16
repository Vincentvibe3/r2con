

use std::{error::{self, Error}, fmt::{self}, net::SocketAddr, time::Duration};
use bytes::BytesMut;
use tokio::{io::{self, AsyncWriteExt}, net::{TcpStream, ToSocketAddrs}, time::sleep};

use crate::rcon_packet::{Packet, PacketType};

#[derive(Debug, Clone)]
pub struct RconAuthError{
    addr:SocketAddr
}

impl fmt::Display for RconAuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not authenticate to {}", self.addr.to_string())
    }
}

impl error::Error for ConnectionClosedError{}

#[derive(Debug, Clone)]
pub struct ConnectionClosedError{
    addr:SocketAddr
}

impl fmt::Display for ConnectionClosedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "connection to {} closed", self.addr.to_string())
    }
}

impl error::Error for RconAuthError {}

pub struct RconClient{
    stream:TcpStream,
}

impl RconClient {
    pub async fn connect<A: ToSocketAddrs>(addr:A, password:&str) -> Result<RconClient, Box<dyn Error>> {
        let stream = TcpStream::connect(addr).await;
        match stream {
            Ok(s) => {
                let mut client = RconClient{
                    stream:s,
                };
                client.login(password).await?;
                return Ok(client);
            }
            Err(e)=>{
                return Err(e.into());
            }
        }
    }

    pub fn get_address(&self) -> io::Result<SocketAddr>{
        return self.stream.peer_addr();
    }

    pub async fn send_command(&mut self, command: &str) -> Result<String, Box<dyn Error >> {
        match self.send(PacketType::Command, command).await {
            Ok(result) => Ok(result),
            Err(e) => {
                let _ = self.stream.shutdown().await;
                Err(e)
            }
        }
    }
 
    async fn login(&mut self, password:&str) -> Result<(), Box<dyn Error>> {
        match self.send(PacketType::Login, password).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = self.stream.shutdown().await;
                Err(e)
            }
        }
    }

    async fn send_packet(&mut self, packet:&Packet) -> Result<(), Box<dyn Error>> {
        let bytes:Vec<u8> = packet.into();
        let bytes_len = bytes.len();
        let mut bytes_written = 0;
        loop {
            self.stream.writable().await?;
            match self.stream.try_write(&bytes[bytes_written..]) {
                Ok(0) => {
                    return Err(ConnectionClosedError{addr:self.stream.peer_addr()?}.into());
                }
                Ok(n) => {
                    bytes_written+=n;
                    if bytes_written == bytes_len {
                        break;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }

    async fn send(&mut self, packet_type:PacketType, payload: &str)-> Result<String, Box<dyn Error>>{
        let mut result_bytes = Vec::<u8>::new();
        let packet = Packet::new(packet_type, payload)?;
        let dummy_packet = Packet::new(PacketType::Response, "")?;

        self.send_packet(&packet).await?;
        // wait before sending a new packet 
        //(minecraft closes the connection otherwise)
        sleep(Duration::from_millis(5)).await; 
        self.send_packet(&dummy_packet).await?;
        sleep(Duration::from_millis(5)).await;

        let mut packet_data = BytesMut::with_capacity(4096);
        'outer: loop {
            self.stream.readable().await?;    
            match self.stream.try_read_buf(&mut packet_data) {
                Ok(0) => {
                    return Err(ConnectionClosedError{addr:self.stream.peer_addr()?}.into());
                },
                Ok(_n) => {},
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            };
            while let Some(packet) = Packet::deserialize(&mut packet_data)? {
                match packet.get_p_type() {
                    PacketType::Invalid => {continue;} // skip invalid packets
                    _ => {}
                }
                let packet_id = packet.get_id();
                if *packet_id == -1 {
                    return Err(RconAuthError {addr:self.stream.peer_addr()?}.into());
                }
                if *packet_id == *(dummy_packet.get_id()) {
                    break 'outer;
                } 
                result_bytes.extend_from_slice(packet.get_body());
            }
        }
        let output = String::from_utf8(result_bytes)?;
        Ok(output)
    }
}
