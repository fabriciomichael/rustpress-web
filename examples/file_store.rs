use rustpress_web::{
    App, Request, Response, TemplateContext,
    session::{FileStore, SessionConfig},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    let store = FileStore::new("storage/sessions.json")?;

    app.use_session_store(SessionConfig::default(), store)?;

    app.get("/login", |req: Request, res: Response| {
        let session = req
            .session()
            .ok_or_else(|| "Gestão de sessões não está activa".to_string())?;

        let mut session = session
            .lock()
            .map_err(|_| "Não foi possível aceder à sessão".to_string())?;

        session
            .insert("username", "Fabricio")
            .map_err(|error| error.to_string())?;

        let context = TemplateContext::new();

        Ok(res.render_template("examples/templates/login.html", &context))
    })?;

    app.get("/perfil", |req: Request, res: Response| {
        let username = req
            .session()
            .and_then(|session| session.lock().ok()?.get::<String>("username"));

        match username {
            Some(username) => {
                let mut context = TemplateContext::new();
                context.set("username", serde_json::json!(username));

                Ok(res.render_template("examples/templates/perfil.html", &context))
            }
            None => Ok(res.html(
                "<h1>Sessão não encontrada.</h1>\
                 <p>Faz login primeiro.</p>",
            )),
        }
    })?;

    println!("RustPress FileStore disponível em http://127.0.0.1:3001");

    app.listen("127.0.0.1:3001").await?;

    Ok(())
}
