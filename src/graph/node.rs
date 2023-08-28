// use tokio::net::TcpStream;

use crate::simulation::{
    engine::{HW_ID_OFFSET, TCP_PORT_OFFSET},
    point::Point,
};

#[derive(Clone, Debug)]
pub struct Node {
    pub id: u32,
    pub hw_id: u32,
    pub tcp_port: u32,

    pub location: Point,
    pub is_router: bool,
    pub is_repeater: bool,
    pub hop_limit: u8, // 3 bytes max in firmware

                       // pub tcp_interface: Option<TcpStream>,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            id: 0,
            hw_id: HW_ID_OFFSET,
            tcp_port: TCP_PORT_OFFSET,

            location: Default::default(),
            is_router: false,
            is_repeater: false,
            hop_limit: 3,
            // tcp_interface: None,
        }
    }
}

impl Node {
    pub fn new(id: u32, location: Point) -> Self {
        Node {
            id,
            hw_id: HW_ID_OFFSET + id,
            tcp_port: TCP_PORT_OFFSET + id,
            location,
            ..Default::default()
        }
    }
}

impl Node {
    pub fn get_full_tcp_address(node: &Node) -> String {
        format!("localhost:{}", node.tcp_port).to_string()
    }
}
