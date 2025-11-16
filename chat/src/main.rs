// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

// #![doc = include_str!("../README.md")]

use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    hash::{Hash, Hasher},
    time::Duration,
};

use futures::stream::StreamExt;
use libp2p::{
    Swarm, gossipsub, identity::Keypair, mdns, noise, swarm::{NetworkBehaviour, SwarmEvent}, tcp, yamux
};
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // libp2pのトレースログを出力可能にする。出力するには環境変数RUST_LOGの設定が必要。
    //  export RUST_LOG=info,[ConnectionHandler::poll]=trace,[NetworkBehaviour::poll]=trace
    //  https://libp2p.github.io/rust-libp2p/metrics_example/index.html#opentelemetry
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let use_quic = if let Some(arg) = std::env::args().nth(1) {
        arg == "quic"
    } else {
        false
    };
    println!("use: quic={}", use_quic);
    let fn_swarm = get_swarm_fn(use_quic);

    // QUICの有無をオプションで変更できるようにしたかったが .with_quic()の有無で型が変わるので止めた
    let mut swarm = fn_swarm.0()?;

    // Create a Gossipsub topic
    let topic = gossipsub::IdentTopic::new("test-net");
    // subscribes to our topic
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    fn_swarm.1(&mut swarm)?;

    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    // Kick it off
    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                // 標準入力を取得したらpublishする
                let line = line.to_uppercase();
                if let Err(e) = swarm
                    .behaviour_mut().gossipsub
                    .publish(topic.clone(), line.as_bytes()) {
                    println!("Publish error: {e:?}");
                }
            }
            event = swarm.select_next_some() => match event {
                // 通信系イベント?

                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => {
                    let msg = String::from_utf8_lossy(&message.data);
                    println!(
                        "Got message: '{msg}' with id: {id} from peer: {peer_id}",
                    );
                    // TODO: ここで"HELLO"と"WORLD"を延々と交換する予定だったがgossipsubの仕様でそれができない。
                    // デフォルトではmessage_bytesをハッシュした値が一致するとDuplicateエラーになるため。
                    if msg == "HELLO" {
                        if let Err(e) = swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topic.clone(), b"WORLD") {
                            println!("Publish error after got message: {e:?}");
                        }
                    } else if msg == "WORLD" {
                        if let Err(e) = swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topic.clone(), b"HELLO") {
                            println!("Publish error after got message: {e:?}");
                        }
                    }
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                }
                _ => {}
            }
        }
    }
}

// QUICの有無を分けたかったら元から分けるのが一番楽。
// ちなみに私はQUICプロトコルのことを知らない。
//  https://ja.wikipedia.org/wiki/QUIC
fn swarm_with_quic() -> Result<Swarm<MyBehaviour>, Box<dyn Error>> {
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new, // noise, tls, plaintext(for test), ...
            yamux::Config::default, // yamux, mplex, ...
        )?
        .with_quic()
        .with_behaviour(my_behaviour)?
        .build();
    Ok(swarm)
}

fn listen_with_quic(swarm: &mut Swarm<MyBehaviour>) -> Result<(), Box<dyn Error>> {
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    Ok(())
}

fn swarm_without_quic() -> Result<Swarm<MyBehaviour>, Box<dyn Error>> {
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new, // noise, tls, plaintext(for test), ...
            yamux::Config::default, // yamux, mplex, ...
        )?
        .with_behaviour(my_behaviour)?
        .build();
    Ok(swarm)
}

fn listen_without_quic(swarm: &mut Swarm<MyBehaviour>) -> Result<(), Box<dyn Error>> {
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    Ok(())
}

fn get_swarm_fn(use_quic: bool) ->
    (
        fn() -> Result<Swarm<MyBehaviour>, Box<dyn Error>>,
        fn(&mut Swarm<MyBehaviour>) -> Result<(), Box<dyn Error>>,
    )
{
    if use_quic {
        (swarm_with_quic, listen_with_quic)
    } else {
        (swarm_without_quic, listen_without_quic)
    }
}

fn my_behaviour(key: &Keypair) -> MyBehaviour {
    behaviour(key).expect("build behaviour for MyBehaviour")
}

fn behaviour(key: &Keypair) -> Result<MyBehaviour, Box<dyn Error>> {
    // To content-address message, we can take the hash of message and use it as an ID.
    let message_id_fn = |message: &gossipsub::Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
    };

    // Set a custom gossipsub configuration
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message
        // signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
        .build()
        .map_err(io::Error::other)?; // Temporary hack because `build` does not return a proper `std::error::Error`.
        //(Copilot提案) .map_err(|e| Box::<dyn Error>::from(e))?; // Map build error into boxed error.

    // build a gossipsub network behaviour
    let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(key.clone()),
        gossipsub_config,
    )?;

    let mdns =
        mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
    Ok(MyBehaviour { gossipsub, mdns })
}
