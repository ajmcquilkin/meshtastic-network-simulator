use meshtastic::connections::stream_api::StreamApi;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    simulation::{
        engine::{HW_ID_OFFSET, TCP_PORT_OFFSET},
        point::Point,
    },
    utils,
};

#[derive(Debug)]
pub struct Node {
    pub id: u32,
    pub hw_id: u32,
    pub tcp_port: u32,

    pub location: Point,
    pub is_router: bool,
    pub is_repeater: bool,
    pub hop_limit: u8, // 3 bytes max in firmware

    pub stream_api: Option<StreamApi>,
    pub cancellation_token: Option<CancellationToken>,
    pub decoded_listener_handle: Option<JoinHandle<()>>,
    pub message_relay_handle: Option<JoinHandle<()>>,
}

impl Node {
    pub fn new(id: u32, location: Point) -> Self {
        Node {
            id,
            hw_id: HW_ID_OFFSET + id,
            tcp_port: TCP_PORT_OFFSET + id,

            location,
            is_router: false,
            is_repeater: false,
            hop_limit: 3,

            stream_api: None,
            cancellation_token: None,
            decoded_listener_handle: None,
            message_relay_handle: None,
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), utils::GenericError> {
        if let Some(stream_api) = self.stream_api.take() {
            stream_api.disconnect().await?;
        }

        if let Some(cancellation_token) = self.cancellation_token.take() {
            cancellation_token.cancel();
        }

        if let Some(decoded_listener_handle) = self.decoded_listener_handle.take() {
            decoded_listener_handle.await?;
        }

        if let Some(message_relay_handle) = self.message_relay_handle.take() {
            message_relay_handle.await?;
        }

        Ok(())
    }

    pub fn get_full_tcp_address(&self) -> String {
        format!("localhost:{}", self.tcp_port).to_string()
    }
}
