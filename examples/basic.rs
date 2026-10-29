use rustpress::{App, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    app.static_files("/assets", "public");

    app.get("/", |_req: Request, res: Response| {
        Ok(res.render_file("views/index.html"))
    })?;

    app.get("/sobre", |_req: Request, res: Response| {
        Ok(res.html(
            "<h1>Sobre o RustPress</h1>\
             <p>Um framework web desenvolvido em Rust.</p>\
             <a href=\"/\">Voltar ao início</a>",
        ))
    })?;

    println!("RustPress disponível em http://127.0.0.1:3000");
    app.listen("127.0.0.1:3000").await?;

    Ok(())
}
