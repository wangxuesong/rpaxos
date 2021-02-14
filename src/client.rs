use crate::{Acceptor, PaxosClient, PaxosInstanceId, Proposer, Value};
use anyhow::{Error, Result};
use futures::future::join_all;
use std::convert::TryFrom;
use tonic::transport::Endpoint;

#[derive(Debug)]
pub struct Client {
    servers: Vec<String>,
    proposer: Proposer,
}

impl Client {
    pub fn new(servers: Vec<String>) -> Self {
        Client {
            servers,
            proposer: Proposer {
                id: Some(PaxosInstanceId {
                    key: "sh".to_string(),
                    version: 0,
                }),
                round: Some(Default::default()),
                value: None,
            },
        }
    }

    pub fn run_round(&self, value: Option<Value>) -> Result<Option<Value>> {
        unimplemented!();
    }

    async fn phase1(&self, svr: Option<Vec<i32>>) -> Result<Option<Value>> {
        let mut svr = if let Some(v) = svr {
            v
        } else {
            (0..self.servers.len() as i32).collect()
        };

        // connect to server
        let mut f = vec![];
        for s in svr {
            let addr = format!("http://{}", self.servers[0].clone());
            let dst = Endpoint::try_from(addr).unwrap();
            let mut client = PaxosClient::connect(dst);
            f.push(client);
        }
        let clients = join_all(f).await;

        // send propose to server
        let mut f = vec![];
        for c in clients {
            let mut client = (c.unwrap());
            let r = client.prepare(self.proposer.clone()).await;
            match r {
                Ok(resp) => {
                    let acc = resp.get_ref();
                    f.push(acc.clone());
                }
                Err(_) => {}
            }
        }

        // collected reply
        let mut max_value = Acceptor {
            round: Some(Default::default()),
            last_round: Some(Default::default()),
            value: None,
        };
        for acc in f {
            if acc.clone().last_round.unwrap().number > self.proposer.round.clone().unwrap().number
            {
                return Err(Error::msg("last_round > round"));
            }

            if acc.clone().round.unwrap().number >= max_value.clone().round.clone().unwrap().number
            {
                max_value = acc;
            }
        }
        let value = max_value.value.clone();
        Ok(value)
    }

    fn phase2(&self, svr: Vec<i32>) -> Result<()> {
        unimplemented!();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;
    use scopeguard::defer;
    use std::thread::JoinHandle;
    use tonic::transport::Server;
    use tonic::Request;
    use triggered::{Listener, Trigger};

    #[tokio::main]
    async fn serve(signal: Listener, address: &str) -> Result<(), tonic::transport::Error> {
        let addr = address.parse().unwrap();

        println!("PaxosServer listening on: {}", addr);

        let service = PaxosService {
            storage: Default::default(),
        };

        let svc = PaxosServer::new(service);

        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addr, async {
                signal.await;
            })
            .await?;

        println!("PaxosServer {} exit.", addr);
        Ok(())
    }

    fn start_server(signal: Listener, address: String) -> JoinHandle<()> {
        let handler = std::thread::spawn(move || {
            let _ = serve(signal, address.as_str());
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        handler
    }

    const BASE_PORT: i32 = 11030;

    struct TestServer {
        count: i32,
        triggers: Vec<Trigger>,
    }

    impl TestServer {
        pub(crate) fn new(count: i32) -> Self {
            TestServer {
                count,
                triggers: Default::default(),
            }
        }

        pub(crate) fn start(&mut self) -> Result<()> {
            for i in 0..self.count {
                let port = BASE_PORT + i;
                let addr = format!("[::1]:{}", port);
                let (trigger, signal) = triggered::trigger();
                start_server(signal, addr);
                self.triggers.push(trigger);
            }
            Ok(())
        }

        pub(crate) fn stop(&mut self) -> Result<()> {
            for t in &self.triggers {
                t.trigger();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            self.triggers.clear();
            Ok(())
        }
    }

    fn server_address(count: i32) -> Vec<String> {
        let mut addrs = vec![];
        for i in 0..count {
            let port = BASE_PORT + i;
            let addr = format!("[::1]:{}", port);
            addrs.push(addr);
        }
        addrs
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_single_propose() {
        let (trigger, signal) = triggered::trigger();
        defer! {
            trigger.trigger();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = start_server(signal, "[::1]:11030".to_string());

        let res = PaxosClient::connect("http://[::1]:11030").await;
        assert!(res.is_ok(), res.unwrap_err().to_string());
        let mut client = res.unwrap();

        // prepare
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
                value: None,
            }
        );

        // accept
        let request = Request::new(Proposer {
            id: Some(PaxosInstanceId {
                key: "sw".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 1,
                proposer_id: 1,
            }),
            value: Some(Value { value: 11 }),
        });

        let res = client.accept(request).await;
        assert!(res.is_ok());
        let resp = res.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            acc,
            &Acceptor {
                round: Some(Default::default()),
                last_round: Some(RoundNum {
                    number: 1,
                    proposer_id: 1,
                }),
                value: None,
            }
        );

        let request = Request::new(Proposer {
            id: Some(PaxosInstanceId {
                key: "sw".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 0,
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
                round: Some(RoundNum {
                    number: 1,
                    proposer_id: 1,
                }),
                last_round: Some(RoundNum {
                    number: 1,
                    proposer_id: 1,
                }),
                value: Some(Value { value: 11 }),
            }
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    pub(super) async fn test_phase1() {
        let mut server = TestServer::new(3);
        server.start();
        defer! {
            server.stop();
        }
        let servers = server_address(3);
        let client = Client::new(servers);
        let res = client.phase1(None).await;
        assert!(res.is_ok(), res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
    }
}
