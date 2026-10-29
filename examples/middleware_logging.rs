use rustpress::{App, MiddlewareResult, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    // Regista informações sobre cada pedido HTTP.
    app.use_middleware_advanced(|request: &Request| {
        println!("[RustPress] {} {}", request.method(), request.path());

        MiddlewareResult::Next
    });

    app.get("/", |_req: Request, res: Response| {
        Ok(res.html("<h1>Bem-vindo ao RustPress!</h1>"))
    })?;

    app.get("/produtos", |_req: Request, res: Response| {
        Ok(res.json(
            serde_json::json!({
                "produtos": [
                    { "id": 1, "nome": "Computador" },
                    { "id": 2, "nome": "Teclado" }
                ]
            })
            .to_string(),
        ))
    })?;

    println!("Servidor disponível em http://127.0.0.1:3000");
    app.listen("127.0.0.1:3000").await?;

    Ok(())
}
