extern crate rpaxos;

use rpaxos::paxos::paxos_server::PaxosServer;
use rpaxos::server::PaxosService;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    let addr = "[::1]:11030".parse().unwrap();

    println!("PaxosServer listening on: {}", addr);

    let service = PaxosService {
        storage: Default::default(),
    };

    let svc = PaxosServer::new(service);

    Server::builder().add_service(svc).serve(addr).await?;

    println!("PaxosServer exit");
    Ok(())
}
