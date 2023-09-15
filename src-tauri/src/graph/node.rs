use meshtastic::{api::ConnectedStreamApi, types::NodeId};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::simulation::point::Point;

use super::types::{HopLimit, Id, TcpPort};

#[derive(Debug)]
pub struct Node {
    pub id: Id,
    pub hw_id: NodeId,
    pub docker_tcp_port: TcpPort,
    pub client_tcp_port: TcpPort,

    pub location: Point,
    pub is_router: bool,
    pub is_repeater: bool,
    pub hop_limit: HopLimit, // 3 bytes max in firmware

    pub stream_api: Option<ConnectedStreamApi>,
    pub cancellation_token: Option<CancellationToken>,

    pub decoded_listener_handle: Option<JoinHandle<()>>,
    pub message_relay_handle: Option<JoinHandle<()>>,
    pub client_forwarding_handle: Option<JoinHandle<()>>,
}

impl Node {
    pub fn new(
        id: Id,
        hw_id: NodeId,
        docker_tcp_port: TcpPort,
        client_tcp_port: TcpPort,
        location: Point,
        hop_limit: Option<HopLimit>,
    ) -> Self {
        Node {
            id,
            hw_id,
            docker_tcp_port,
            client_tcp_port,

            location,
            is_router: false,
            is_repeater: false,
            hop_limit: hop_limit.unwrap_or_default(),

            stream_api: None,
            cancellation_token: None,

            decoded_listener_handle: None,
            message_relay_handle: None,
            client_forwarding_handle: None,
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), anyhow::Error> {
        if let Some(stream_api) = self.stream_api.take() {
            stream_api
                .disconnect()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
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

        if let Some(client_forwarding_handle) = self.client_forwarding_handle.take() {
            client_forwarding_handle.await?;
        }

        Ok(())
    }

    pub fn get_full_docker_tcp_address(&self) -> String {
        format!("localhost:{}", self.docker_tcp_port).to_string()
    }

    pub fn get_full_client_tcp_address(&self) -> String {
        format!("localhost:{}", self.client_tcp_port).to_string()
    }
}
