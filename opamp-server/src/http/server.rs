use crate::http::{
    handler::{RequestHandler, RequestHandlerOpts, RouterService},
    Callbacks, OpAMPServer, Result, Settings, StartSettings,
};
use hyper::server::Server;
use std::{net::SocketAddr, sync::Arc};

struct HttpServer {
    settings: Settings,
    worker_threads: usize,
    max_blocking_threads: usize,
}

impl HttpServer {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            worker_threads: 0,
            max_blocking_threads: 0,
        }
    }

    /// Run the inner Hyper `HyperServer` (HTTP1/HTTP2) forever on the current thread
    /// using the given configuration.
    async fn start_server<T: Callbacks + std::marker::Send + std::marker::Sync + 'static>(
        &mut self,
        settings: StartSettings,
        callbacks: T,
    ) -> Result<()> {
        let service = RouterService::new(RequestHandler {
            opts: Arc::from(RequestHandlerOpts {
                compression: self.settings.enable_compression,
                callbacks,
            }),
        });

        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let server = Server::bind(&addr).serve(service);

        server.await.unwrap();

        todo!()
    }
}

impl OpAMPServer for HttpServer {
    fn attach<T: Callbacks + std::marker::Sync>(
        &mut self,
        settings: Settings,
        callbacks: T,
    ) -> Result<()> {
        todo!()
    }

    fn start<T: Callbacks + std::marker::Send + std::marker::Sync + 'static>(
        &mut self,
        settings: StartSettings,
        callbacks: T,
    ) -> Result<()> {
        // Tokio runtime to block on async func
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(self.worker_threads)
            .max_blocking_threads(self.max_blocking_threads)
            .thread_name("opamp-server")
            .enable_all()
            .build()
            .unwrap()
            .block_on(self.start_server(settings, callbacks))
    }

    fn stop(&self) -> Result<()> {
        todo!()
    }
}
