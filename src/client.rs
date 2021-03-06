use crate::{Acceptor, PaxosClient, PaxosInstanceId, Proposer, RoundNum, Value};
use anyhow::{Error, Result};
use futures::future::join_all;
use std::convert::TryFrom;
use tonic::transport::{Channel, Endpoint};

#[derive(Debug, Clone, Default)]
pub struct Propose {
    proposer: Proposer,
    servers: Vec<String>,
    context: Vec<PaxosClient<Channel>>,
}

impl Propose {
    pub fn new(servers: Vec<String>, key: String, value: Option<Value>, id: i64) -> Self {
        Propose {
            servers,
            proposer: Proposer {
                id: Some(PaxosInstanceId { key, version: 0 }),
                round: Some(RoundNum {
                    number: 0,
                    proposer_id: id,
                }),
                value: value,
            },
            ..Default::default()
        }
    }

    async fn phase1(&mut self, svr: Option<Vec<i32>>) -> Result<Option<Value>> {
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
        let mut acc = vec![];
        for res in clients {
            match res {
                Ok(c) => acc.push(c),
                Err(_) => {}
            }
        }

        self.phase1_with_client(acc).await
    }

    async fn phase1_with_client(
        &mut self,
        clients: Vec<PaxosClient<Channel>>,
    ) -> Result<Option<Value>, Error> {
        if self.context.len() == 0 {
            return Err(Error::msg("not enough quorum"));
        }

        // send propose to server
        let mut f = vec![];
        for c in clients {
            let mut client = c;
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
        let round = self.proposer.round.clone().unwrap().number;
        for acc in f {
            let last_round = acc.clone().last_round.unwrap().number;
            if round < last_round {
                return Err(Error::msg("last_round > round"));
            }

            let value_round = acc.clone().round.unwrap().number;
            if round == last_round && round > value_round {
                return Err(Error::msg("round > value_round"));
            }

            if last_round >= max_value.clone().last_round.clone().unwrap().number {
                max_value = acc;
            }
        }
        // round = value_round 修复
        let last_round = max_value.clone().last_round.unwrap().number;
        let value_round = max_value.clone().round.unwrap().number;
        if round == last_round && round == value_round {
            self.proposer.value = max_value.clone().value;
        }
        // round > last_round 更新
        if round > last_round {
            self.proposer.value = None;
        }
        let value = self.proposer.value.clone();
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
        let mut acc = vec![];
        for res in clients {
            match res {
                Ok(c) => acc.push(c),
                Err(_) => {}
            }
        }

        self.phase2_with_client(acc).await
    }

    async fn phase2_with_client(
        &mut self,
        clients: Vec<PaxosClient<Channel>>,
    ) -> Result<(), Error> {
        // send propose to server
        let mut f = vec![];
        for c in clients {
            let mut client = c;
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

    #[cfg(test)]
    fn set_proposer(&mut self, proposer: Proposer) -> Result<()> {
        self.proposer = proposer;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<Option<Value>> {
        let v = self.phase1_with_client(self.context.clone()).await?;
        self.proposer.value = if let Some(_) = v.clone() {
            v // 修复
        } else {
            self.proposer.value.clone() // 更新
        };
        self.phase2_with_client(self.context.clone()).await?;
        Ok(self.proposer.value.clone())
    }

    /// 临时函数，设置连接 [`Acceptor`] 的 [`PaxosClient`]
    pub fn set_context(&mut self, context: Vec<PaxosClient<Channel>>) -> Result<()> {
        self.context = context;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Client {
    id: i64,
    servers: Vec<String>,
    acceptors: Vec<PaxosClient<Channel>>,
    propose: Propose,
}

impl Client {
    pub fn new(servers: Vec<String>, id: i64) -> Self {
        Client {
            id,
            propose: Propose::new(servers.clone(), "key".to_string(), None, id),
            servers,
            ..Default::default()
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        // connect to server
        let mut f = vec![];
        for s in self.servers.clone() {
            let addr = format!("http://{}", s.clone());
            let dst = Endpoint::try_from(addr).unwrap();
            let client = PaxosClient::connect(dst);
            f.push(client);
        }
        let results = join_all(f).await;
        let quorum = self.servers.len() / 2 + 1;
        let mut acceptors = vec![];
        for acc in results {
            match acc {
                Ok(c) => {
                    acceptors.push(c);
                }
                Err(_) => {}
            }
        }
        self.acceptors = acceptors.clone();
        if acceptors.len() >= quorum {
            Ok(())
        } else {
            Err(Error::msg("not enough quorum"))
        }
    }

    pub async fn run_propose(
        &mut self,
        key: String,
        value: Option<Value>,
    ) -> Result<Option<Value>> {
        let mut prop = Propose::new(self.servers.clone(), key, value.clone(), self.id);
        prop.set_context(self.acceptors.clone())?;
        return prop.run().await;
    }

    #[cfg(test)]
    async fn phase1(&mut self, svr: Option<Vec<i32>>) -> Result<Option<Value>> {
        return self.propose.phase1(svr).await;
    }

    #[cfg(test)]
    async fn phase2(&mut self, svr: Option<Vec<i32>>) -> Result<()> {
        return self.propose.phase2(svr).await;
    }

    #[cfg(test)]
    fn set_proposer(&mut self, proposer: Proposer) -> Result<()> {
        self.propose.set_proposer(proposer)
    }

    #[cfg(test)]
    fn proposer(&mut self) -> Proposer {
        self.propose.proposer.clone()
    }
}

#[cfg(test)]
mod test {
    // use super::*;
    use crate::*;
    use anyhow::Result;
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
        let mut client = Client::new(servers, 0);
        assert!(client.connect().await.is_ok());

        // round > value_round
        let mut rnd = 5i64;
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: rnd,
                proposer_id: 0,
            }),
            value: None,
        };
        client.set_proposer(prop.clone()).unwrap();
        let res = phase1(&mut client).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
        {
            let mut p = prop.clone();
            p.value = Some(Value { value: 11 });
            client.set_proposer(p).unwrap();
            assert!(phase2(&mut client).await.is_ok());
        }
        // round = 5 && value = 11

        // round = value_round
        let mut prop = prop.clone();
        prop.round = Some(RoundNum {
            number: rnd,
            proposer_id: 0,
        });
        client.set_proposer(prop.clone()).unwrap();
        let res = phase1(&mut client).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert_eq!(value, Some(Value { value: 11 }));
        {
            let mut p = prop.clone();
            p.value = Some(Value { value: 03 });
            client.set_proposer(p).unwrap();
            assert!(phase2(&mut client).await.is_ok());
        }
        // round = 5 && value = 11

        // round < last_round
        let mut prop = prop.clone();
        rnd = 3;
        prop.round = Some(RoundNum {
            number: rnd,
            proposer_id: 0,
        });
        client.set_proposer(prop.clone()).unwrap();
        let res = phase1(&mut client).await;
        assert!(res.is_err());
        {
            let mut p = prop.clone();
            p.value = Some(Value { value: 04 });
            client.set_proposer(p).unwrap();
            let res = phase2(&mut client).await;
            assert!(res.is_err(), "{}", res.err().unwrap().to_string());
        }
        // round = 5 && value = 11

        // round = last_round && round > value_round
        let mut prop = prop.clone();
        rnd = 6;
        prop.round = Some(RoundNum {
            number: rnd,
            proposer_id: 0,
        });
        client.set_proposer(prop.clone()).unwrap();
        let res = phase1(&mut client).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert_eq!(value, None);
        // last_round = 6 && value_round = 5
        {
            let mut p = prop.clone();
            p.value = Some(Value { value: 05 });
            // round = 6
            client.set_proposer(p).unwrap();
            let res = phase1(&mut client).await;
            assert!(res.is_err());
        }
    }

    async fn phase1(client: &mut Client) -> Result<Option<Value>> {
        client.propose.set_context(client.acceptors.clone())?;
        client
            .propose
            .phase1_with_client(client.acceptors.clone())
            .await
    }

    async fn phase2(client: &mut Client) -> Result<()> {
        // client.propose.set_context(client.acceptors.clone())?;
        client
            .propose
            .phase2_with_client(client.acceptors.clone())
            .await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    pub(super) async fn test_phase2() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        let mut client = Client::new(servers, 0);
        assert!(client.connect().await.is_ok());
        let res = phase1(&mut client).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
        let res = phase2(&mut client).await;
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
        let alice_id = 11i64;
        let mut alice = Client::new(servers.clone(), alice_id);
        assert!(alice.connect().await.is_ok());
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 1,
                proposer_id: alice_id,
            }),
            value: None,
        };
        alice.set_proposer(prop).unwrap();
        let res = phase1(&mut alice).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 1,
                proposer_id: alice_id,
            }),
            value: Some(Value { value: 3 }),
        };
        alice.set_proposer(prop).unwrap();
        let res = phase2(&mut alice).await;
        assert!(res.is_ok());
        // assert!(alice.proposer.value.is_none());

        let bob_id = 88i64;
        let mut bob = Client::new(servers, bob_id);
        assert!(bob.connect().await.is_ok());
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 2,
                proposer_id: bob_id,
            }),
            value: None,
        };
        bob.set_proposer(prop).unwrap();
        let res = phase1(&mut bob).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert_eq!(value, None);
        let prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 2,
                proposer_id: bob_id,
            }),
            value: Some(Value { value: 4 }),
        };
        bob.set_proposer(prop).unwrap();
        let res = phase2(&mut bob).await;
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
        let alice_id = 11i64;
        let mut alice = Client::new(servers.clone(), alice_id);
        assert!(alice.connect().await.is_ok());
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
        let res = phase1(&mut alice).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());

        // bob proposer round=2
        let bob_id = 88i64;
        let mut bob = Client::new(servers, bob_id);
        assert!(bob.connect().await.is_ok());
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
        let res = phase1(&mut bob).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert!(value.is_none());

        // alice proceed phase 2, failed;
        alice_prop.value = Some(Value { value: 3 });
        alice.set_proposer(alice_prop).unwrap();
        let res = phase2(&mut alice).await;
        assert!(res.is_err());

        // bob proceed phase 2, succeed;
        bob_prop.value = Some(Value { value: 11 });
        bob.set_proposer(bob_prop).unwrap();
        let res = phase2(&mut bob).await;
        assert!(res.is_ok());
        assert_eq!(bob.proposer().value, Some(Value { value: 11 }));
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
        let alice_id = 11i64;
        let mut alice = Client::new(servers.clone(), alice_id);
        assert!(alice.connect().await.is_ok());
        assert!(alice.propose.set_context(alice.acceptors.clone()).is_ok());
        let mut alice_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 1,
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
        let bob_id = 88i64;
        let mut bob = Client::new(servers, bob_id);
        assert!(bob.connect().await.is_ok());
        assert!(bob.propose.set_context(bob.acceptors.clone()).is_ok());
        let mut bob_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 2,
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
        assert_eq!(bob.proposer().value, Some(Value { value: 11 }));

        // alice propose with round=3
        alice_prop = Proposer {
            id: Some(PaxosInstanceId {
                key: "sh".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 3,
                proposer_id: alice_id,
            }),
            value: None,
        };
        alice.set_proposer(alice_prop.clone()).unwrap();
        let res = alice.phase1(Some(vec![0, 1])).await;
        assert!(res.is_ok(), "{}", res.err().unwrap().to_string());
        let value = res.unwrap();
        assert_eq!(value, None);
        // alice proceed phase 2, succeed;
        alice_prop.value = Some(Value { value: 3 });
        alice.set_proposer(alice_prop).unwrap();
        let res = alice.phase2(Some(vec![0, 1])).await;
        assert!(res.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
    pub(super) async fn test_run_round() {
        let mut server = TestServer::new(3);
        assert!(server.start().is_ok());
        defer! {
            let _ = server.stop();
        }
        let servers = server_address(3);
        // alice proposer round=1
        let alice_id = 11i64;
        let mut alice = Client::new(servers.clone(), alice_id);
        assert!(alice.connect().await.is_ok());
        let res = alice
            .run_propose("sh".to_string(), Some(Value { value: 11 }))
            .await;
        assert!(res.is_ok());
    }
}
