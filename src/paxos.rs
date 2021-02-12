/// 每一轮的编号，全局唯一
/// number: 本地单调递增计数器
/// proposer_id: 全局唯一 ID
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RoundNum {
    #[prost(int64, tag = "1")]
    pub number: i64,
    #[prost(int64, tag = "2")]
    pub proposer_id: i64,
}
/// 保存的值，此处为整型
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Value {
    #[prost(int64, tag = "1")]
    pub value: i64,
}
/// 一个 Paxos 实例，对应一次完整的投票
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PaxosInstanceId {
    #[prost(string, tag = "1")]
    pub key: ::prost::alloc::string::String,
    #[prost(int64, tag = "2")]
    pub version: i64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Acceptor {
    #[prost(message, optional, tag = "1")]
    pub round: ::core::option::Option<RoundNum>,
    #[prost(message, optional, tag = "2")]
    pub last_round: ::core::option::Option<RoundNum>,
    #[prost(message, optional, tag = "3")]
    pub value: ::core::option::Option<Value>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Proposer {
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<PaxosInstanceId>,
    #[prost(message, optional, tag = "2")]
    pub round: ::core::option::Option<RoundNum>,
    #[prost(message, optional, tag = "3")]
    pub value: ::core::option::Option<Value>,
}
#[doc = r" Generated client implementations."]
pub mod paxos_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct PaxosClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl PaxosClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> PaxosClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn prepare(
            &mut self,
            request: impl tonic::IntoRequest<super::Proposer>,
        ) -> Result<tonic::Response<super::Acceptor>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/paxos.Paxos/Prepare");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn accept(
            &mut self,
            request: impl tonic::IntoRequest<super::Proposer>,
        ) -> Result<tonic::Response<super::Acceptor>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/paxos.Paxos/Accept");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for PaxosClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for PaxosClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "PaxosClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod paxos_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with PaxosServer."]
    #[async_trait]
    pub trait Paxos: Send + Sync + 'static {
        async fn prepare(
            &self,
            request: tonic::Request<super::Proposer>,
        ) -> Result<tonic::Response<super::Acceptor>, tonic::Status>;
        async fn accept(
            &self,
            request: tonic::Request<super::Proposer>,
        ) -> Result<tonic::Response<super::Acceptor>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct PaxosServer<T: Paxos> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: Paxos> PaxosServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T, B> Service<http::Request<B>> for PaxosServer<T>
    where
        T: Paxos,
        B: HttpBody + Send + Sync + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/paxos.Paxos/Prepare" => {
                    #[allow(non_camel_case_types)]
                    struct PrepareSvc<T: Paxos>(pub Arc<T>);
                    impl<T: Paxos> tonic::server::UnaryService<super::Proposer> for PrepareSvc<T> {
                        type Response = super::Acceptor;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Proposer>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).prepare(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = PrepareSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/paxos.Paxos/Accept" => {
                    #[allow(non_camel_case_types)]
                    struct AcceptSvc<T: Paxos>(pub Arc<T>);
                    impl<T: Paxos> tonic::server::UnaryService<super::Proposer> for AcceptSvc<T> {
                        type Response = super::Acceptor;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Proposer>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).accept(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = AcceptSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: Paxos> Clone for PaxosServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: Paxos> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Paxos> tonic::transport::NamedService for PaxosServer<T> {
        const NAME: &'static str = "paxos.Paxos";
    }
}
