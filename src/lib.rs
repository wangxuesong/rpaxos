mod paxos;
mod server;

pub use crate::paxos::paxos_client::PaxosClient;
pub use crate::paxos::paxos_server::PaxosServer;
pub use crate::paxos::*;
pub use crate::server::PaxosService;
