use rustpress::{App, Request, Response, TemplateContext};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    app.static_files("/assets", "public");

    let mut contexto = TemplateContext::new();

    contexto.set("titulo", json!("RustPress"));
    contexto.set("nome", json!("Fabrício"));
    contexto.set("autenticado", json!(true));

    app.get("/", move |_req: Request, res: Response| {
        res.render_view(
            "views/pages/index.html",
            "views/layouts/main.html",
            "views/partials",
            &contexto,
        )
    })
    .expect("Não foi possível registar a rota GET /perfil");

    // Cada página pode receber o seu próprio contexto, independente das restantes rotas.
    let mut profile: TemplateContext = TemplateContext::new();

    profile.set("nome", json!("Luca"));
    profile.set("idade", json!(19));

    // A captura por valor transfere o contexto para o handler registado.
    app.get("/perfil", move |_req: Request, res: Response| {
        res.render_view(
            "views/pages/perfil.html",
            "views/layouts/main.html",
            "views/partials",
            &profile,
        )
    })
    .expect("Não foi possível registar a rota GET /perfil");

    // Inicia o servidor no endereço indicado; esta chamada permanece activa enquanto
    // o servidor estiver em execução e propaga erros de inicialização ou ligação.
    app.listen("127.0.0.1:3000").await?;
    Ok(())
}
