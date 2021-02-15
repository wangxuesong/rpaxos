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
                value: Some(Value { value: 11 }),
            },
        }
    }

    pub fn run_round(&self, value: Option<Value>) -> Result<Option<Value>> {
        unimplemented!();
    }

    async fn phase1(&self, svr: Option<Vec<i32>>) -> Result<Option<Value>> {
        let svr = if let Some(v) = svr {
            v
        } else {
            (0..self.servers.len() as i32).collect()
        };

        // connect to server
        let mut f = vec![];
        for s in svr {
            let addr = format!("http://{}", self.servers[s as usize].clone());
            let dst = Endpoint::try_from(addr)?;
            let client = PaxosClient::connect(dst);
            f.push(client);
        }
        let clients = join_all(f).await;

        // send propose to server
        let mut f = vec![];
        for c in clients {
            let mut client = c?;
            let r = client.prepare(self.proposer.clone()).await;
            match r {
                Ok(resp) => {
                    let acc = resp.get_ref();
                    f.push(acc.clone());
                }
                Err(e) => {
                    return Err(Error::new(e));
                }
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

    async fn phase2(&mut self, svr: Option<Vec<i32>>) -> Result<()> {
        let svr = if let Some(v) = svr {
            v
        } else {
            (0..self.servers.len() as i32).collect()
        };

        // connect to server
        let mut f = vec![];
        for s in svr {
            let addr = format!("http://{}", self.servers[s as usize].clone());
            let dst = Endpoint::try_from(addr).unwrap();
            let client = PaxosClient::connect(dst);
            f.push(client);
        }
        let clients = join_all(f).await;

        // send propose to server
        let mut f = vec![];
        for c in clients {
            let mut client = c?;
            let r = client.accept(self.proposer.clone()).await;
            match r {
                Ok(resp) => {
                    let acc = resp.get_ref();
                    f.push(acc.clone());
                }
                Err(e) => {
                    return Err(Error::new(e));
                }
            }
        }

        // collected reply
        let max_value = Acceptor {
            round: Some(Default::default()),
            last_round: Some(Default::default()),
            value: None,
        };
        let rnd = self.proposer.clone().round.clone().unwrap().number;
        let quorum = self.servers.len() / 2 + 1;
        let mut count = 0usize;
        for acc in f {
            // 有其他更大的 round 请求，本次请求失败
            if acc.clone().last_round.unwrap().number <= rnd {
                // 本次请求有效，记录有效节点数
                count += 1;
                if count >= quorum {
                    // 多数派同意请求
                    return Ok(());
                }
            }
        }

        Err(Error::msg("not enough quorum"))
    }

    fn set_proposer(&mut self, proposer: Proposer) -> Result<()> {
        self.proposer = proposer;
        Ok(())
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
    async fn test_single_propose_single_server() {
        let (trigger, signal) = triggered::trigger();
        defer! {
            trigger.trigger();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = start_server(signal, "[::1]:11038".to_string());

        let res = PaxosClient::connect("http://[::1]:11038").await;
        assert!(res.is_ok(), "{}", res.unwrap_err().to_string());
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    pub(super) async fn test_phase1() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        let client = Client::new(servers);
        let res = client.phase1(None).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    pub(super) async fn test_phase2() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        let mut client = Client::new(servers);
        let res = client.phase1(None).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
        let res = client.phase2(None).await;
        assert!(res.is_ok());
        // assert!(client.proposer.value.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
    pub(super) async fn test_double_client_normal_scenes() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        let mut alice = Client::new(servers.clone());
        let alice_id = 11i64;
        let mut rnd = 1i64;
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: alice_id,
            }),
            value: None,
        };
        alice.set_proposer(prop).unwrap();
        let res = alice.phase1(None).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: alice_id,
            }),
            value: Some(Value { value: 3 }),
        };
        alice.set_proposer(prop).unwrap();
        let res = alice.phase2(None).await;
        assert!(res.is_ok());
        // assert!(alice.proposer.value.is_none());

        let mut bob = Client::new(servers);
        let bob_id = 88i64;
        rnd += 1;
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: bob_id,
            }),
            value: None,
        };
        bob.set_proposer(prop).unwrap();
        let res = bob.phase1(None).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert_eq!(value, Some(Value { value: 3 }));
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: bob_id,
            }),
            value: Some(Value { value: 4 }),
        };
        bob.set_proposer(prop).unwrap();
        let res = bob.phase2(None).await;
        assert!(res.is_ok());
        // assert_eq!(bob.proposer.value, Some(Value { value: 3 }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
    pub(super) async fn test_double_client_exception_scenes() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        // alice proposer round=1
        let mut alice = Client::new(servers.clone());
        let alice_id = 11i64;
        let mut rnd = 1i64;
        let mut alice_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: alice_id,
            }),
            value: None,
        };
        alice.set_proposer(alice_prop.clone()).unwrap();
        let res = alice.phase1(None).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());

        // bob proposer round=2
        let mut bob = Client::new(servers);
        let bob_id = 88i64;
        rnd += 1;
        let mut bob_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: bob_id,
            }),
            value: None,
        };
        bob.set_proposer(bob_prop.clone()).unwrap();
        let res = bob.phase1(None).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());

        // alice proceed phase 2, failed;
        alice_prop.value = Some(Value { value: 3 });
        alice.set_proposer(alice_prop).unwrap();
        let res = alice.phase2(None).await;
        assert!(res.is_err());

        // bob proceed phase 2, succeed;
        bob_prop.value = Some(Value { value: 11 });
        bob.set_proposer(bob_prop).unwrap();
        let res = bob.phase2(None).await;
        assert!(res.is_ok());
        assert_eq!(bob.proposer.value, Some(Value { value: 11 }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
    pub(super) async fn test_double_client_partition_exception_scenes() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        // alice proposer round=1
        let mut alice = Client::new(servers.clone());
        let alice_id = 11i64;
        let mut rnd = 1i64;
        let mut alice_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: alice_id,
            }),
            value: None,
        };
        alice.set_proposer(alice_prop.clone()).unwrap();
        let res = alice.phase1(Some(vec![0, 1])).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());

        // bob proposer round=2
        let mut bob = Client::new(servers);
        let bob_id = 88i64;
        rnd += 1;
        let mut bob_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: bob_id,
            }),
            value: None,
        };
        bob.set_proposer(bob_prop.clone()).unwrap();
        let res = bob.phase1(Some(vec![1, 2])).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());

        // alice proceed phase 2, failed;
        alice_prop.value = Some(Value { value: 3 });
        alice.set_proposer(alice_prop).unwrap();
        let res = alice.phase2(Some(vec![0, 1])).await;
        assert!(res.is_err());

        // bob proceed phase 2, succeed;
        bob_prop.value = Some(Value { value: 11 });
        bob.set_proposer(bob_prop).unwrap();
        let res = bob.phase2(Some(vec![1, 2])).await;
        assert!(res.is_ok());
        assert_eq!(bob.proposer.value, Some(Value { value: 11 }));

        // alice propose with round=3
        rnd += 1;
        alice_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: alice_id,
            }),
            value: None,
        };
        alice.set_proposer(alice_prop.clone()).unwrap();
        let res = alice.phase1(Some(vec![0, 1])).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert_eq!(value, Some(Value { value: 11 }));
        // alice proceed phase 2, succeed;
        alice_prop.value = Some(Value { value: 3 });
        alice.set_proposer(alice_prop).unwrap();
        let res = alice.phase2(Some(vec![0, 1])).await;
        assert!(res.is_ok());
    }
}
