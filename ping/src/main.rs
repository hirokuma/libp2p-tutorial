use std::{error::Error, time::Duration};

use futures::prelude::*;
use libp2p::{noise, ping, tcp, yamux, Multiaddr, swarm::SwarmEvent};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // swarmのbuildにはTransportとBehaviourがいる
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp( // Transport
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| ping::Behaviour::default())? // Behavior(ping: 15秒ごとに送信, 20秒以内に受信)
        // ここ以下はオプション的なもの
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(Duration::from_secs(60_u64)) // 接続期間。終わるとSwarmEvent::ConnectionClosedが発生。
        })
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    if let Some(addr) = std::env::args().nth(1) {
        let remote: Multiaddr = addr.parse()?;
        swarm.dial(remote)?;
        println!("Dialed: {addr}");
    }

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {address:?}"),
            SwarmEvent::Behaviour(event) => println!("{event:?}"),
            SwarmEvent::ConnectionClosed { peer_id: _, connection_id: _, endpoint: _, num_established: _, cause: _ } => println!("disconnected"),
            _ => {},
        }
    }
}
