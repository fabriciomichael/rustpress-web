use rustpress_web::{App, HandlerError, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    // Defines global error handling.
    app.set_error_handler(|error: &HandlerError, context| {
        eprintln!(
            "[RustPress] Erro {} em {} {}: {}",
            error.status(),
            context.method(),
            context.path(),
            error
        );

        Response::new().status(error.status()).html(format!(
            "<h1>Erro {}</h1><p>Ocorreu um problema ao processar o pedido.</p>",
            error.status()
        ))
    });

    // Test route that intentionally triggers a 500 error.
    app.get("/erro", |_req: Request, _res: Response| {
        Err(HandlerError::new("Erro interno de teste"))
    })?;

    println!("Servidor disponível em http://127.0.0.1:3001");
    app.listen("127.0.0.1:3001").await?;

    Ok(())
}
