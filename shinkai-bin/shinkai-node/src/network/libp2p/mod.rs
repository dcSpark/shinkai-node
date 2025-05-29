use libp2p::{
    core::upgrade,
    identity::Keypair,
    noise,
    request_response::{self, Codec, Event, ProtocolSupport, Config},
    swarm::{NetworkBehaviour, Swarm},
    tcp, yamux, Multiaddr, PeerId, Transport, SwarmBuilder,
};
use futures::prelude::*;
use async_trait::async_trait;
use std::error::Error;

#[derive(Clone)]
pub struct ShinkaiProtocol();

#[derive(Clone, Debug)]
pub struct ShinkaiRequest(pub Vec<u8>);

#[derive(Clone, Debug)]
pub struct ShinkaiResponse(pub Vec<u8>);

impl AsRef<str> for ShinkaiProtocol {
    fn as_ref(&self) -> &str {
        "/shinkai/1"
    }
}

#[derive(Clone)]
pub struct ShinkaiCodec;

#[async_trait]
impl Codec for ShinkaiCodec {
    type Protocol = ShinkaiProtocol;
    type Request = ShinkaiRequest;
    type Response = ShinkaiResponse;

    async fn read_request<T>(&mut self, _: &ShinkaiProtocol, io: &mut T) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        Ok(ShinkaiRequest(buf))
    }

    async fn read_response<T>(&mut self, _: &ShinkaiProtocol, io: &mut T) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        Ok(ShinkaiResponse(buf))
    }

    async fn write_request<T>(&mut self, _: &ShinkaiProtocol, io: &mut T, ShinkaiRequest(data): ShinkaiRequest) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        io.write_all(&data).await
    }

    async fn write_response<T>(&mut self, _: &ShinkaiProtocol, io: &mut T, ShinkaiResponse(data): ShinkaiResponse) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        io.write_all(&data).await
    }
}

#[derive(NetworkBehaviour)]
pub struct ComposedBehaviour {
    pub req_res: request_response::Behaviour<ShinkaiCodec>,
}

#[derive(Debug)]
pub enum ComposedEvent {
    ReqRes(Event<ShinkaiRequest, ShinkaiResponse>),
}

impl From<Event<ShinkaiRequest, ShinkaiResponse>> for ComposedEvent {
    fn from(event: Event<ShinkaiRequest, ShinkaiResponse>) -> Self {
        ComposedEvent::ReqRes(event)
    }
}

pub struct Libp2pNetwork {
    pub swarm: Swarm<ComposedBehaviour>,
}

impl Libp2pNetwork {
    pub async fn new(keypair: Keypair, listen: Multiaddr) -> Result<Self, Box<dyn Error>> {
        let transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::Config::new(&keypair)?)
            .multiplex(yamux::Config::default())
            .boxed();

        let mut cfg = Config::default();
        cfg = cfg.with_request_timeout(std::time::Duration::from_secs(10));
        let behaviour = ComposedBehaviour {
            req_res: request_response::Behaviour::with_codec(ShinkaiCodec, std::iter::once((ShinkaiProtocol(), ProtocolSupport::Full)), cfg),
        };
        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_other_transport(|_| transport)?
            .with_behaviour(|_| behaviour)?
            .with_swarm_config(|c| c.with_idle_connection_timeout(std::time::Duration::from_secs(60)))
            .build();
        swarm.listen_on(listen)?;
        Ok(Self { swarm })
    }

    pub fn peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn send(&mut self, peer: PeerId, data: Vec<u8>) {
        let req = ShinkaiRequest(data);
        self.swarm.behaviour_mut().req_res.send_request(&peer, req);
    }
}
