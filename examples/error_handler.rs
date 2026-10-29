
use rustpress::{App, HandlerError, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    // Define o tratamento global de erros.
    app.set_error_handler(|error: &HandlerError, context| {
        eprintln!(
            "[RustPress] Erro {} em {} {}: {}",
            error.status(),
            context.method(),
            context.path(),
            error
        );

        Response::new()
            .status(error.status())
            .html(format!(
                "<h1>Erro {}</h1><p>Ocorreu um problema ao processar o pedido.</p>",
                error.status()
            ))
    });

    // Rota de teste que provoca intencionalmente um erro 500.
    app.get("/erro", |_req: Request, _res: Response| {
        Err(HandlerError::new("Erro interno de teste"))
    })?;

    println!("Servidor disponível em http://127.0.0.1:3001");
    app.listen("127.0.0.1:3001").await?;

    Ok(())
}
