#[cfg(test)]
mod test {
    use crate::{
        Acceptor, PaxosClient, PaxosInstanceId, PaxosServer, PaxosService, Proposer, RoundNum,
    };
    use scopeguard::defer;
    use std::thread::JoinHandle;
    use tonic::transport::Server;
    use tonic::Request;
    use triggered::Listener;

    #[tokio::main]
    async fn serve(singal: Listener) -> Result<(), tonic::transport::Error> {
        let addr = "[::1]:11030".parse().unwrap();

        println!("PaxosServer listening on: {}", addr);

        let service = PaxosService {
            storage: Default::default(),
        };

        let svc = PaxosServer::new(service);

        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addr, async {
                singal.await;
            })
            .await?;

        println!("PaxosServer exit");
        Ok(())
    }

    fn start_server(signal: Listener) -> JoinHandle<()> {
        let handler = std::thread::spawn(move || {
            let _ = serve(signal);
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        handler
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_single_propose() {
        let (trigger, signal) = triggered::trigger();
        defer! {
            trigger.trigger();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = start_server(signal);

        let res = PaxosClient::connect("http://[::1]:11030").await;
        assert!(res.is_ok());
        let mut client = res.unwrap();

        let request = Request::new(Proposer {
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

        let res = client.prepare(request).await;
        assert!(res.is_ok());
        let resp = res.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            acc,
            &Acceptor {
                round: Some(Default::default()),
                last_round: Some(Default::default()),
                value: None
            }
        );
    }
}
