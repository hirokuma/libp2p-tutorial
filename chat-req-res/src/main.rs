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

use chat_req_res::server;

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
    // 1番目は自分のポート番号。必須。
    let my_port = std::env::args().nth(1).expect("Listen port number");
    // 2番目は接続先のポート番号。ないなら接続しに行かない。
    let connect_port = std::env::args().nth(2).unwrap_or("".to_string());

    tokio::spawn(server::start("127.0.0.1:8000".to_string()));

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
    swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{my_port}").parse()?)?;

   if connect_port.len() > 0 {
        let remote: Multiaddr = format!("/ip4/127.0.0.1/tcp/{connect_port}").parse()?;
        swarm.dial(remote)?;
        println!("Dialed");
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
                    let res_msg = request.data.to_uppercase();
                    if let Err(e) = swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, ChatResponse{data: res_msg}) {
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
