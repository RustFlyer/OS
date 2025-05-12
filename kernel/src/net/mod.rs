use strum::FromRepr;

pub mod addr;
pub mod sock;
pub mod socket;
pub mod sockopt;

#[derive(FromRepr, Debug, PartialEq, Eq, Clone, Copy)]
pub enum SocketType {
    /// TCP
    STREAM = 1,
    /// UDP
    DGRAM = 2,
    RAW = 3,
    RDM = 4,
    SEQPACKET = 5,
    DCCP = 6,
    PACKET = 10,
}
