mod paxos;
mod server;

use crate::paxos::paxos_client::PaxosClient;
use crate::paxos::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = PaxosClient::connect("http://[::1]:11030").await?;

    let request = tonic::Request::new(Proposer {
        id: Some(PaxosInstanceId {
            key: "sw".to_string(),
            version: 0,
        }),
        round: Some(RoundNum {
            number: 1,
            proposer_id: 1,
        }),
        value: Some(Value { value: 0 }),
    });

    let response = client.prepare(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
}
