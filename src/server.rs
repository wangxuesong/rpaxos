use crate::paxos::paxos_server::Paxos;
use crate::paxos::{Acceptor, Proposer};
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
        let ver = id.version;
        let round = proposer.round.as_ref().unwrap();

        {
            // for lock storage
            let storage = self.storage.lock().unwrap();
            if storage.contains_key(&key) {
                let mut value = storage.get(&key).cloned().unwrap();
                if round.number > value.round.as_ref().unwrap().number {
                    value.round = Some(round.clone());
                }
                Ok(Response::new(value))
            } else {
                let acc = Acceptor {
                    round: proposer.round.clone(),
                    last_round: proposer.round.clone(),
                    value: proposer.value.clone(),
                };
                let mut s = storage;
                s.insert(key, acc.clone());
                Ok(Response::new(acc))
            }
        } // unlock storage
    }

    async fn accept(&self, request: Request<Proposer>) -> Result<Response<Acceptor>, Status> {
        unimplemented!()
    }
}

mod tests {
    use super::*;
    use crate::paxos::{PaxosInstanceId, RoundNum, Value};
    use tokio::runtime::Runtime;

    #[test]
    fn test_prepare_phase_1() {
        let service = PaxosService {
            storage: Default::default(),
        };
        let r0 = Request::new(Proposer {
            id: Some(PaxosInstanceId {
                key: "test".to_string(),
                version: 0,
            }),
            round: Some(RoundNum {
                number: 0,
                proposer_id: 0,
            }),
            value: Some(Value { value: 11 }),
        });
        let result = service.prepare(r0);
        let r = Runtime::new().unwrap().block_on(result);
        assert!(r.is_ok());
        let resp = r.unwrap();
        let acc = resp.get_ref();
        assert_eq!(
            &Acceptor {
                round: Some(RoundNum {
                    number: 0,
                    proposer_id: 0
                }),
                last_round: Some(RoundNum {
                    number: 0,
                    proposer_id: 0
                }),
                value: Some(Value { value: 11 })
            },
            acc
        );
    }
}
