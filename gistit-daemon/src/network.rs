//! The network module
#![allow(clippy::missing_errors_doc)]

use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::string::ToString;
use std::task::Poll;

use either::Either;
use gistit_ipc::{self, Bridge, Instruction, Server, ServerResponse};
use log::{debug, error, info, warn};

use libp2p::core::either::EitherError;
use libp2p::core::PeerId;
use libp2p::futures::future::poll_fn;
use libp2p::futures::StreamExt;
use libp2p::multiaddr::multiaddr;
use libp2p::swarm::{ProtocolsHandlerUpgrErr, SwarmBuilder, SwarmEvent};
use libp2p::{tokio_development_transport, Swarm};

use libp2p::identify::{IdentifyEvent, IdentifyInfo};
use libp2p::kad::{protocol, record::Key, QueryId};
use libp2p::ping::Failure;
use libp2p::request_response::RequestId;

use crate::behaviour::{Behaviour, Event, Request};
use crate::config::Config;
use crate::event::{handle_kademlia, handle_request_response};
use crate::Result;

/// The main event loop
pub struct Node {
    pub swarm: Swarm<Behaviour>,
    pub bridge: Bridge<Server>,

    pub pending_dial: HashSet<PeerId>,

    /// Pending kademlia queries to get providers
    pub pending_get_providers: HashSet<QueryId>,

    pub pending_start_providing: HashSet<QueryId>,
    pub to_provide: HashMap<Key, Vec<u8>>,

    pub pending_request_file: HashSet<RequestId>,

    /// Stack of request file (`key`) events
    pub to_request: Vec<(Key, HashSet<PeerId>)>,
}

impl Node {
    pub async fn new(config: Config) -> Result<Self> {
        let behaviour = Behaviour::new(&config)?;
        let transport = tokio_development_transport(config.keypair)?;

        let mut swarm = SwarmBuilder::new(transport, behaviour, config.peer_id)
            .executor(Box::new(|fut| {
                tokio::task::spawn(fut);
            }))
            .build();
        let bridge = gistit_ipc::server(&config.runtime_dir)?;

        // Listen on all interfaces
        let address = multiaddr!(Ip4(Ipv4Addr::new(0, 0, 0, 0)), Tcp(0_u16));
        swarm.listen_on(address)?;

        Ok(Self {
            swarm,
            bridge,
            pending_dial: HashSet::default(),
            pending_start_providing: HashSet::default(),
            pending_get_providers: HashSet::default(),
            pending_request_file: HashSet::default(),

            to_provide: HashMap::default(),
            to_request: Vec::default(),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                swarm_event = self.swarm.next() => self.handle_swarm_event(
                    swarm_event.expect("stream not to end")).await?,

                bridge_event = self.bridge.recv() => self.handle_bridge_event(bridge_event?).await?,

                request_event = poll_fn(|_| {
                    self.to_request.pop().map_or(Poll::Pending, Poll::Ready)
                }) => self.handle_request_event(request_event).await,
            }
        }
    }

    async fn handle_request_event(&mut self, event: (Key, HashSet<PeerId>)) {
        let (key, providers) = event;

        for p in providers {
            let request_id = self
                .swarm
                .behaviour_mut()
                .request_response
                .send_request(&p, Request(key.to_vec()));
            self.pending_request_file.insert(request_id);
        }
    }

    #[allow(clippy::type_complexity)]
    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<
            Event,
            EitherError<
                EitherError<
                    EitherError<
                        EitherError<
                            EitherError<ProtocolsHandlerUpgrErr<std::io::Error>, std::io::Error>,
                            std::io::Error,
                        >,
                        Either<
                            ProtocolsHandlerUpgrErr<
                                EitherError<
                                    impl std::error::Error + Send,
                                    impl std::error::Error + Send,
                                >,
                            >,
                            void::Void,
                        >,
                    >,
                    ProtocolsHandlerUpgrErr<std::io::Error>,
                >,
                Failure,
            >,
        >,
    ) -> Result<()> {
        match event {
            SwarmEvent::Behaviour(Event::Identify(IdentifyEvent::Received {
                peer_id,
                info:
                    IdentifyInfo {
                        listen_addrs,
                        protocols,
                        ..
                    },
            })) => {
                debug!("Identify: {:?}", listen_addrs);
                if protocols
                    .iter()
                    .any(|p| p.as_bytes() == protocol::DEFAULT_PROTO_NAME)
                {
                    for addr in listen_addrs {
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                }
            }

            SwarmEvent::Behaviour(Event::Kademlia(event)) => handle_kademlia(self, event),

            SwarmEvent::Behaviour(Event::RequestResponse(event)) => {
                handle_request_response(self, event).await?;
            }

            SwarmEvent::NewListenAddr { address, .. } => {
                let peer_id = self.swarm.local_peer_id().to_string();
                info!("Daemon: Listening on {:?}, {:?}", address, peer_id);

                self.bridge.connect_blocking()?;
                self.bridge
                    .send(Instruction::Response(ServerResponse::PeerId(peer_id)))
                    .await?;
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!("Connection established {:?}", peer_id);
                if endpoint.is_dialer() {
                    self.pending_dial.remove(&peer_id);
                }
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: maybe_peer_id,
                error,
                ..
            } => {
                error!("Outgoing connection error: {:?}", error);
                if let Some(peer_id) = maybe_peer_id {
                    self.pending_dial.remove(&peer_id);
                }
            }
            ev => {
                debug!("other event: {:?}", ev);
            }
        }
        Ok(())
    }

    async fn handle_bridge_event(&mut self, instruction: Instruction) -> Result<()> {
        match instruction {
            Instruction::Provide { hash, data } => {
                warn!("Instruction: Provide gistit {}", hash);
                let key = Key::new(&hash);

                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(key.clone())
                    .expect("to start providing");

                self.pending_start_providing.insert(query_id);
                self.to_provide.insert(key, data);
            }

            Instruction::Get { hash } => {
                warn!("Instruction: Get providers for {}", hash);
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(Key::new(&hash));
                self.pending_get_providers.insert(query_id);
            }

            Instruction::Status => {
                warn!("Instruction: Status");

                let listeners: Vec<String> =
                    self.swarm.listeners().map(ToString::to_string).collect();
                let network_info = self.swarm.network_info();

                self.bridge.connect_blocking()?;
                self.bridge
                    .send(Instruction::Response(ServerResponse::Status {
                        peer_count: network_info.num_peers(),
                        pending_connections: network_info.connection_counters().num_pending(),
                        listeners,
                    }))
                    .await?;
            }

            Instruction::Shutdown => {
                warn!("Exiting...");
                std::process::exit(0);
            }

            Instruction::Response(_) => (),
        }
        Ok(())
    }
}
