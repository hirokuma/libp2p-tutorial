use std::error::Error;

use futures::stream::StreamExt;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, noise,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use serde::{Deserialize, Serialize};
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;

// Request/Responseで送受信するメッセージ型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatRequest {
    data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatResponse {
    data: String,
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    request_response: request_response::cbor::Behaviour<ChatRequest, ChatResponse>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // libp2pのトレースログを出力可能にする。出力するには環境変数RUST_LOGの設定が必要。
    //  export RUST_LOG=info,[ConnectionHandler::poll]=trace,[NetworkBehaviour::poll]=trace
    //  https://libp2p.github.io/rust-libp2p/metrics_example/index.html#opentelemetry
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,     // noise, tls, plaintext(for test), ...
            yamux::Config::default, // yamux, mplex, ...
        )?
        .with_behaviour(|_| MyBehaviour {
            request_response: request_response::cbor::Behaviour::<ChatRequest, ChatResponse>::new(
                [(StreamProtocol::new("/chat-chat/1"), ProtocolSupport::Full)],
                request_response::Config::default(),
            ),
        })?
        .build();

    let peer_id = swarm.local_peer_id();
    println!("My peer ID: {}", peer_id);

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // 引数があれば接続先として扱う
    if let Some(addr) = std::env::args().nth(1) {
        let remote: Multiaddr = addr.parse()?;
        swarm.dial(remote)?;
        println!("Dialed: {addr}");
    } else {
        println!("Not dialed");
    }

    println!("Enter messages via STDIN and they will be sent to connected peers");

    // gossipsubの仕様でmessageIdが同じになるとpublish()でDuplicateエラーになる。
    // message_id_fn の実装でmessageIdの計算方法を変更できる。
    let mut connected_peer_id = PeerId::random();
    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                // https://deepwiki.com/search/requestresponse-behaviour_3732d1e4-4ee6-49e1-8805-e10941b9b463?mode=fast
                println!("input: {line}");
                let id = swarm.behaviour_mut()
                    .request_response
                    .send_request(&connected_peer_id, ChatRequest{data: line});
                println!("send request id: {}", id);
            },
            event = swarm.select_next_some() => match event {
                // 通信系イベント?

                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                },
                SwarmEvent::ConnectionEstablished {peer_id, connection_id: _, endpoint: _, num_established: _, concurrent_dial_errors: _, established_in: _ } => {
                    connected_peer_id = peer_id;
                    println!("connected: {}", connected_peer_id);
                },
                SwarmEvent::ConnectionClosed { peer_id: _, connection_id: _, endpoint: _, num_established: _, cause: _ } => println!("disconnected"),
                // SwarmEvent::Behaviour(event) => println!("{event:?}"),
                SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(request_response::Event::Message {
                    peer: _,
                    connection_id: _,
                    message: request_response::Message::Request { request_id: _, request, channel },
                })) => {
                    println!("request: {}", request.data);
                    if let Err(e) = swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, ChatResponse{data: "WORLD".to_string()}) {
                        println!("response send error: {e:?}");
                    } else {
                        println!("send response");
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(request_response::Event::Message {
                    peer: _,
                    connection_id: _,
                    message: request_response::Message::Response { request_id: _, response }
                })) => {
                    println!("response: {}", response.data);
                },

                _ => {}
            }
        }
    }
}
