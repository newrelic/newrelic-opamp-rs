use crate::http::{transport::Transport, Callbacks};
use hyper::{service::Service, Body, Request, Response};
use std::convert::Infallible;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{net::SocketAddr, sync::Arc};

pub(crate) struct RequestHandlerOpts<T: Callbacks> {
    pub(crate) compression: bool,
    pub(crate) callbacks: T,
}

/// It defines the main request handler used by the Hyper service request.
pub(crate) struct RequestHandler<T: Callbacks> {
    pub(crate) opts: Arc<RequestHandlerOpts<T>>,
}

impl<T: Callbacks> RequestHandler<T> {
    fn handle<'a>(
        &'a self,
        req: &'a mut Request<Body>,
        remote_addr: Option<SocketAddr>,
    ) -> impl Future<Output = Result<Response<Body>, hyper::Error>> + Send + 'a {
        async move {
            let resp = Response::new(Body::empty());
            Ok(resp)
        }
    }
}

pub(crate) struct RequestService<T: Callbacks> {
    handler: Arc<RequestHandler<T>>,
    remote_addr: Option<SocketAddr>,
}

impl<T: Callbacks + std::marker::Sync + std::marker::Send + 'static> Service<Request<Body>>
    for RequestService<T>
{
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Response<Body>, hyper::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), hyper::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let handler = self.handler.clone();
        let remote_addr = self.remote_addr;
        Box::pin(async move { handler.handle(&mut req, remote_addr).await })
    }
}

/// It defines a Hyper service request builder.
pub(crate) struct RequestServiceBuilder<T: Callbacks> {
    handler: Arc<RequestHandler<T>>,
}

impl<T: Callbacks> RequestServiceBuilder<T> {
    pub fn new(handler: RequestHandler<T>) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    pub fn build(&self, remote_addr: Option<SocketAddr>) -> RequestService<T> {
        RequestService {
            handler: self.handler.clone(),
            remote_addr,
        }
    }
}

/// It defines the router service which is the main entry point for Hyper Server.
pub(crate) struct RouterService<T: Callbacks> {
    builder: RequestServiceBuilder<T>,
}

impl<T: Callbacks> RouterService<T> {
    pub fn new(handler: RequestHandler<T>) -> Self {
        Self {
            builder: RequestServiceBuilder::new(handler),
        }
    }
}

impl<T, U> Service<&T> for RouterService<U>
where
    T: Transport + Send + 'static,
    U: Callbacks,
{
    type Response = RequestService<U>;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, conn: &T) -> Self::Future {
        ready(Ok(self.builder.build(conn.remote_addr())))
    }
}
