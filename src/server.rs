use std::{convert::Infallible, error::Error};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::router::Router;

type ServerError = Box<dyn Error + Send + Sync>;

pub(crate) async fn listen(address: &str, router: Router) -> Result<(), ServerError> {
    let listener = TcpListener::bind(address).await?;

    println!("RustPress a escutar em http://{address}");

    serve(listener, router).await
}

pub(crate) async fn serve(listener: TcpListener, router: Router) -> Result<(), ServerError> {
    loop {
        let (stream, _) = listener.accept().await?;
        let router = router.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let service = service_fn(move |request| {
                let router = router.clone();

                async move { Ok::<_, Infallible>(router.handle(request)) }
            });

            if let Err(error) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Erro na ligação HTTP: {error}");
            }
        });
    }
}
