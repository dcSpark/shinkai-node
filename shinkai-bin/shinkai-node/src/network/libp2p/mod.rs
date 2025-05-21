use libp2p::{
    core::upgrade,
    identity::{Keypair, ed25519},
    noise,
    request_response::{RequestResponse, RequestResponseCodec, RequestResponseEvent, ProtocolSupport, RequestResponseMessage, RequestResponseConfig},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use futures::{prelude::*, StreamExt};
use async_trait::async_trait;
use std::error::Error;

#[derive(Clone)]
pub struct ShinkaiProtocol();

#[derive(Clone)]
pub struct ShinkaiRequest(pub Vec<u8>);

#[derive(Clone)]
pub struct ShinkaiResponse(pub Vec<u8>);

impl libp2p::request_response::ProtocolName for ShinkaiProtocol {
    fn protocol_name(&self) -> &[u8] {
        b"/shinkai/1"
    }
}

pub struct ShinkaiCodec;

#[async_trait]
impl RequestResponseCodec for ShinkaiCodec {
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
#[behaviour(out_event = "ComposedEvent")]
pub struct ComposedBehaviour {
    pub req_res: RequestResponse<ShinkaiCodec>,
}

#[derive(Debug)]
pub enum ComposedEvent {
    ReqRes(RequestResponseEvent<ShinkaiRequest, ShinkaiResponse>),
}

impl From<RequestResponseEvent<ShinkaiRequest, ShinkaiResponse>> for ComposedEvent {
    fn from(event: RequestResponseEvent<ShinkaiRequest, ShinkaiResponse>) -> Self {
        ComposedEvent::ReqRes(event)
    }
}

pub struct Libp2pNetwork {
    pub swarm: Swarm<ComposedBehaviour>,
}

impl Libp2pNetwork {
    pub async fn new(keypair: Keypair, listen: Multiaddr) -> Result<Self, Box<dyn Error>> {
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&keypair)
            .expect("Signing libp2p noise static keypair failed");
        let transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(yamux::Config::default())
            .boxed();

        let mut cfg = RequestResponseConfig::default();
        cfg.set_connection_keep_alive(std::time::Duration::from_secs(10));
        let behaviour = ComposedBehaviour {
            req_res: RequestResponse::new(ShinkaiCodec, std::iter::once((ShinkaiProtocol(), ProtocolSupport::Full)), cfg),
        };
        let mut swarm = Swarm::with_tokio_executor(transport, behaviour, keypair.public().to_peer_id());
        Swarm::listen_on(&mut swarm, listen)?;
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
