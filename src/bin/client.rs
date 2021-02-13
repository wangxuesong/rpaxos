extern crate rpaxos;

use rpaxos::{PaxosClient, PaxosInstanceId, Proposer, RoundNum};

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
        value: None,
    });

    let response = client.prepare(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
}
