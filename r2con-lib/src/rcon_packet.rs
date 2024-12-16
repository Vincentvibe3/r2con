use std::error::Error;

use bytes::{Buf, BufMut, BytesMut};
use rand::Rng;

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum PacketType {
    Response = 0i32,
    Command = 2i32,
    Login = 3i32,
    Invalid = -2i32
}

impl PacketType {
    pub fn from_i32(i:i32) -> PacketType{
        return match i {
            0 => PacketType::Response,
            2 => PacketType::Command,
            3 => PacketType::Login,
            _ => PacketType::Invalid
        }
    }
}

pub struct Packet {
    size: i32,
    id: i32,
    p_type: PacketType,
    body: Vec<u8>
}


impl Packet{

    pub fn new(packet_type:PacketType, payload:&str) -> Result<Packet, Box<dyn Error>>{
        let mut rng = rand::thread_rng();
        let payload_len = payload.len()+1; // add null terminator
        let size = i32::try_from(payload_len+9)?;
        let packet = Packet{
            size: size,
            id: rng.gen::<i32>(),
            p_type: packet_type,
            body: payload.as_bytes().to_vec()
        };
        return Ok(packet);
    } 

    pub fn get_size(&self) -> &i32{
        return &self.size;
    }

    pub fn get_id(&self) -> &i32{
        return &self.id;
    }

    pub fn get_p_type(&self) -> &PacketType{
        return &self.p_type;
    }

    pub fn get_body(&self) -> &Vec<u8>{
        return &self.body;
    }

    pub fn deserialize(buf:&mut BytesMut) -> Result<Option<Self>, Box<dyn Error>>{
        let mut buf_len = buf.len();
        if buf_len > 4 {
            let packet_size = buf.get_i32_le();
            buf_len-=4;
            let buffer_size = i32::try_from(buf_len)?;
            if packet_size <= buffer_size {
                let payload_size = usize::try_from(packet_size-10)?;
                let id = buf.get_i32_le();
                let p_type_i32= buf.get_i32_le();
                let p_type = PacketType::from_i32(p_type_i32);
                let mut payload_buf = Vec::with_capacity(payload_size);
                let take = buf.take(payload_size);
                payload_buf.put(take);
                // buf.advance(payload_size);
                buf.get_u16(); // remove the null terminators
                return Ok(Some(Packet{
                    size:packet_size,
                    id: id,
                    p_type: p_type,
                    body:payload_buf
                }));
            }
        }
        Ok(None)
    }
}

impl From<Packet> for Vec<u8> {

    fn from(value: Packet) -> Vec<u8> {
        let mut result = if let Ok(s_usize) = usize::try_from(&value.size+4) {
            Vec::<u8>::with_capacity(s_usize)
        } else {
            Vec::<u8>::new()
        };
        result.extend_from_slice(&value.size.to_le_bytes());
        result.extend_from_slice(&value.id.to_le_bytes());
        result.extend_from_slice(&((value.p_type as i32).to_le_bytes()));
        result.extend_from_slice(&value.body);
        result.push(0); // add terminators
        result.push(0); // add terminators
        return result;
    }
}

impl From<&Packet> for Vec<u8> {

    fn from(value: &Packet) -> Vec<u8> {
        let mut result = if let Ok(s_usize) = usize::try_from(&value.size+4) {
            Vec::<u8>::with_capacity(s_usize)
        } else {
            Vec::<u8>::new()
        };
        result.extend_from_slice(&value.size.to_le_bytes());
        result.extend_from_slice(&value.id.to_le_bytes());
        result.extend_from_slice(&((value.p_type as i32).to_le_bytes()));
        result.extend_from_slice(&value.body);
        result.push(0); // add terminators
        result.push(0); // add terminators
        return result;
    }
}