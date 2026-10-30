//! Integração do router com Hyper e Tokio.
//!
//! Este módulo interno abre ou recebe um listener TCP, aceita ligações e
//! encaminha pedidos HTTP para o router. Cada ligação é processada de forma
//! assíncrona.

use crate::logger::Logger;
use std::{convert::Infallible, error::Error, time::Instant};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::router::Router;

type ServerError = Box<dyn Error + Send + Sync>;

pub(crate) async fn listen(
    address: &str,
    router: Router,
    logger: Logger,
) -> Result<(), ServerError> {
    let listener = TcpListener::bind(address).await?;

    logger.info(&format!("Server listening in http://{address}"));

    serve(listener, router, logger).await
}

pub(crate) async fn serve(
    listener: TcpListener,
    router: Router,
    logger: Logger,
) -> Result<(), ServerError> {
    loop {
        let (stream, _) = listener.accept().await?;

        let router = router.clone();
        let logger = logger.clone();
        let connection_logger = logger.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let service = service_fn(move |request| {
                let router = router.clone();
                let logger = logger.clone();

                async move {
                    let method = request.method().clone();
                    let path = request.uri().path().to_owned();
                    let start = Instant::now();

                    let response = router.handle(request);

                    logger.info(&format!(
                        "{} {} → {} ({} ms)",
                        method,
                        path,
                        response.status().as_u16(),
                        start.elapsed().as_millis()
                    ));

                    Ok::<_, Infallible>(response)
                }
            });

            if let Err(error) = http1::Builder::new().serve_connection(io, service).await {
                connection_logger.error(&format!("Error in HTTP connection: {error}"));
            }
        });
    }
}
