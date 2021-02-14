use crate::paxos::paxos_server::Paxos;
use crate::paxos::{Acceptor, Proposer, RoundNum};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct PaxosService {
    pub storage: Arc<Mutex<HashMap<String, Acceptor>>>,
}

#[tonic::async_trait]
impl Paxos for PaxosService {
    async fn prepare(&self, request: Request<Proposer>) -> Result<Response<Acceptor>, Status> {
        let proposer = request.get_ref();
        let id = proposer.id.clone().unwrap();
        let key = id.key;
        let request_round = proposer.round.as_ref().unwrap();
        let request_value = proposer.value.clone();

        // for lock storage
        {
            let mut storage = self.storage.lock().unwrap();
            if storage.contains_key(&key) {
                let value = storage.get(&key).cloned().unwrap();
                if request_round.number > value.round.as_ref().unwrap().number {
                    // 保存请求中的 round 到 last_round
                    let new_value = Acceptor {
                        round: Some(request_round.clone()),
                        last_round: Some(request_round.clone()),
                        value: request_value,
                    };
                    storage.insert(key, new_value);
                }
                Ok(Response::new(value))
            } else {
                let acc = Acceptor {
                    round: Some(RoundNum::default()),
                    last_round: Some(RoundNum::default()),
                    value: None,
                };
                let mut acceptor = acc.clone();
                acceptor.last_round = proposer.round.clone();
                storage.insert(key, acceptor);
                Ok(Response::new(acc))
            }
        } // unlock storage
    }

    async fn accept(&self, request: Request<Proposer>) -> Result<Response<Acceptor>, Status> {
        let proposer = request.get_ref();
        let id = proposer.id.clone().unwrap();
        let key = id.key;
        let request_round = proposer.round.as_ref().unwrap();
        let request_value = proposer.value.clone().unwrap();

        // for lock storage
        {
            let mut storage = self.storage.lock().unwrap();
            let acc = storage.get(&key).cloned().unwrap();
            if request_round.number >= acc.last_round.clone().unwrap().number {
                let mut new_value = acc.clone();
                new_value.round = Some(request_round.clone());
                new_value.value = Some(request_value);
                new_value.last_round = Some(request_round.clone());
                storage.insert(key, new_value);
            }
            Ok(Response::new(acc))
        } // unlock storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paxos::{PaxosInstanceId, RoundNum, Value};
    use tokio_test::block_on;

    #[test]
    fn test_prepare() {
        let service = PaxosService {
            storage: Default::default(),
        };
        let r0 = Request::new(Proposer {
            id: Some(PaxosInstanceId {
                key: "test".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 1,
                proposer_id: 0,
            }),
            value: Some(Value { value: 11 }),
        });
        let result = service.prepare(r0);
        let r = block_on(result);
        assert!(r.is_ok());
        let resp = r.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            &Acceptor {
                round: Some(RoundNum::default()),
                last_round: Some(RoundNum {
                    number: 0,
                    proposer_id: 0,
                }),
                value: None,
            },
            acc
        );

        let r1 = Request::new(Proposer {
            id: Some(PaxosInstanceId {
                key: "test".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 2,
                proposer_id: 0,
            }),
            value: Some(Value { value: 03 }),
        });
        let r = block_on(service.prepare(r1));
        assert!(r.is_ok());
        let resp = r.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            acc,
            &Acceptor {
                round: Some(RoundNum {
                    number: 0,
                    proposer_id: 0,
                }),
                last_round: Some(RoundNum {
                    number: 1,
                    proposer_id: 0,
                }),
                value: None,
            }
        );
    }

    #[test]
    fn test_accept() {
        let service = PaxosService {
            storage: Default::default(),
        };
        let proposer = Proposer {
            id: Some(PaxosInstanceId {
                key: "test1".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 1,
                proposer_id: 0,
            }),
            value: Some(Value { value: 11 }),
        };
        let r0 = Request::new(proposer.clone());
        let r = block_on(service.prepare(r0));
        assert!(r.is_ok());
        let resp = r.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            &Acceptor {
                round: Some(RoundNum::default()),
                last_round: Some(RoundNum {
                    number: 0,
                    proposer_id: 0,
                }),
                value: None,
            },
            acc
        );

        let r = block_on(service.accept(Request::new(proposer.clone())));
        assert!(r.is_ok());
        let resp = r.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            &Acceptor {
                round: Some(RoundNum {
                    number: 0,
                    proposer_id: 0,
                }),
                last_round: Some(RoundNum {
                    number: 1,
                    proposer_id: 0,
                }),
                value: None,
            },
            acc
        );
    }
}
