<p align="center">
  <img src="assets/rustpress-logo.svg" alt="RustPress — HTTP framework for Rust" width="520">
</p>

<p align="center">
  <strong>Um framework HTTP leve para Rust, inspirado na simplicidade do Express.js.</strong>
</p>

<p align="center">
  <a href="https://github.com/fabriciomichael/rustpress-web">GitHub</a> ·
  <a href="docs/ARCHITECTURE.md">Arquitectura</a> ·
  <a href="docs/CONTRIBUTING.md">Contribuir</a>
</p>

---

## Sobre o projecto

**RustPress** é um framework HTTP assíncrono para Rust, construído sobre **Hyper** e **Tokio**. O seu objectivo é oferecer uma API directa para criar servidores web e APIs, com rotas, middleware, respostas HTTP, ficheiros estáticos, templates e tratamento centralizado de erros.

O projecto procura combinar uma interface familiar para quem conhece o Express.js com os princípios de segurança, desempenho e controlo de tipos da linguagem Rust.

> O RustPress encontra-se em desenvolvimento. Consulte a API e os exemplos existentes neste repositório antes de assumir que uma funcionalidade de outros frameworks já está disponível.

## Funcionalidades actuais

- **Servidor HTTP assíncrono:** Hyper 1 e Tokio.
- **Rotas:** GET, POST, PUT, PATCH e DELETE.
- **Parâmetros dinâmicos:** padrões como `/utilizadores/:id`.
- **Query strings:** consulta de parâmetros como `?pagina=2&limite=10`.
- **Request e Response:** tipos próprios para trabalhar com pedidos e respostas HTTP.
- **Middleware:** middleware simples e cadeia avançada com continuação, resposta imediata ou erro.
- **Tratamento de erros:** erros devolvidos por handlers, handler global e páginas de erro 404, 405 e 500.
- **Respostas:** texto, HTML, JSON, conteúdo binário, headers e códigos de estado.
- **Ficheiros estáticos:** publicação de CSS, JavaScript, imagens e outros ficheiros a partir de uma pasta configurada.
- **Templates HTML:** variáveis, condições, ciclos, layouts e partials.
- **Logger:** níveis DEBUG, INFO, WARN e ERROR, com saída colorida e registo de pedidos do servidor.
- **Testes:** testes unitários e testes de integração para funcionalidades existentes.

## Requisitos

- Rust estável com suporte à edição **2024**.
- Cargo (instalado com Rust).
- Sistema operativo suportado por Tokio e Hyper.

## Instalação e execução do repositório

Clone o projecto:

```bash
git clone https://github.com/fabriciomichael/rustpress-web.git
cd rustpress-web
```

Compile e execute os testes:

```bash
cargo build
cargo test
```

Execute um exemplo:

```bash
cargo run --example basic
```

O servidor de demonstração fica disponível em `http://127.0.0.1:3000`.

> **Nota sobre a utilização como dependência:** o código do repositório tem o nome de pacote `rustpress`, mas os metadados de publicação e o URL de repositório ainda devem ser alinhados antes de apresentar `cargo add rustpress` como instrução de instalação de uma versão publicada. Para desenvolver com este repositório, use os comandos acima.

## Arranque rápido

O exemplo seguinte regista rotas, lê um parâmetro dinâmico e responde a um pedido POST. Os handlers GET e POST actuais devolvem `Result<Response, HandlerError>`, por isso é possível propagar erros com `?`.

```rust
use rustpress::{App, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    app.get("/", |_req: Request, res: Response| {
        Ok(res.html("<h1>Bem-vindo ao RustPress</h1>"))
    })?;

    app.get("/utilizadores/:id", |req: Request, res: Response| {
        let id = req.param("id").unwrap_or("desconhecido");
        Ok(res.json(
            serde_json::json!({ "id": id }).to_string()
        ))
    })?;

    app.post("/utilizadores", |_req: Request, res: Response| {
        Ok(res.status(201).json(
            serde_json::json!({
                "mensagem": "Utilizador criado"
            }).to_string()
        ))
    })?;

    app.listen("127.0.0.1:3000").await?;
    Ok(())
}
```

Guarde o código, por exemplo, em `examples/basic_api.rs`, e execute-o com:

```bash
cargo run --example basic_api
```

## Rotas HTTP

As rotas são registadas através da instância de `App`. Os métodos disponíveis são:

| Método | Registo | Handler com erro |
|---|---|---|
| GET | `app.get(path, handler)` | Sim |
| POST | `app.post(path, handler)` | Sim |
| PUT | `app.put(path, handler)` | `app.put_fallible(path, handler)` |
| PATCH | `app.patch(path, handler)` | `app.patch_fallible(path, handler)` |
| DELETE | `app.delete(path, handler)` | `app.delete_fallible(path, handler)` |

O registo de uma rota devolve `Result<(), String>`; use `?` ou trate o erro para detectar padrões de rota inválidos.

### Parâmetros de rota

```rust
app.get("/produtos/:id", |req: Request, res: Response| {
    let id = req.param("id").unwrap_or("0");
    Ok(res.send(format!("Produto: {id}")))
})?;
```

### Query string

```rust
// URL de exemplo: /produtos?pagina=2
app.get("/produtos", |req: Request, res: Response| {
    let pagina = req.query("pagina").unwrap_or("1");
    Ok(res.send(format!("Página: {pagina}")))
})?;
```

### O que o Request disponibiliza

A API pública actual de `Request` inclui:

- `method()`: método HTTP;
- `path()`: caminho sem a query string;
- `header("Content-Type")`: acesso a um cabeçalho;
- `param("id")`: parâmetro dinâmico da rota;
- `query("pagina")`: parâmetro da query string;
- `context()`: cópia dos metadados da requisição para o handler de erros.

**Limitação actual importante:** a API pública de `Request` ainda não expõe um método para ler o corpo bruto do pedido nem para descodificar automaticamente formulários `application/x-www-form-urlencoded` ou `multipart/form-data`. Por isso, embora seja possível registar uma rota POST, isso não significa que o RustPress já processe campos de formulários ou uploads de ficheiros. Não confunda o suporte ao método HTTP POST com suporte a FormData.

## Respostas HTTP

O tipo `Response` permite construir respostas encadeando métodos:

```rust
app.get("/sobre", |_req: Request, res: Response| {
    Ok(res
        .status(200)
        .header("X-App", "RustPress")
        .html("<h1>Sobre o projecto</h1>"))
})?;
```

Métodos disponíveis:

- `Response::new()`: cria uma resposta com estado 200.
- `.status(code)`: define o código HTTP.
- `.send(text)`: envia texto simples.
- `.html(html)`: envia HTML.
- `.json(json_string)`: envia uma string JSON.
- `.error(code, message)`: cria uma resposta de erro em texto.
- `.json_error(code, message)`: cria uma resposta de erro JSON.
- `.header(name, value)`: define um cabeçalho.
- `.send_bytes(bytes)`: envia conteúdo binário.
- `.render_file(path)`: carrega um ficheiro HTML sem processar variáveis.
- `.render_template(path, context)`: carrega e processa um template com contexto.
- `.render(page, layout, partials_dir, context)`: renderiza uma página com layout e partials, em modo estrito.
- `Response::error_page(...)`: cria uma página de erro com layout e partials.

O método `.json(...)` recebe texto JSON; use `serde_json::json!(...).to_string()` para gerar JSON válido. O RustPress não converte automaticamente qualquer estrutura Rust em JSON através deste método.

## Middleware

### Middleware simples

Um middleware simples recebe uma referência ao pedido. Devolve `None` para deixar o processamento continuar ou `Some(Response)` para interromper o pedido e responder imediatamente.

```rust
app.use_middleware(|req: &Request| {
    if req.path() == "/admin" {
        Some(Response::new().status(403).send("Acesso proibido"))
    } else {
        None
    }
});
```

### Middleware avançado

O middleware avançado devolve um dos valores de `MiddlewareResult`:

- `MiddlewareResult::Next`: continua para o middleware seguinte ou para a rota;
- `MiddlewareResult::Respond(response)`: interrompe a cadeia e envia a resposta;
- `MiddlewareResult::Error(error)`: encaminha o erro para o tratamento central.

```rust
use rustpress::{App, MiddlewareResult, Request, Response};

let mut app = App::new();

app.use_middleware_advanced(|req: &Request| {
    if req.path().starts_with("/privado") {
        MiddlewareResult::Respond(
            Response::new().status(403).send("Acesso negado")
        )
    } else {
        MiddlewareResult::Next
    }
});
```

O exemplo completo de logging através de middleware está em `examples/middleware_logging.rs`.

## Tratamento de erros

O RustPress disponibiliza `HandlerError` para que um handler possa devolver uma falha em vez de uma resposta normal.

### Criar e devolver um erro

```rust
use rustpress::{App, HandlerError, Request, Response};

let mut app = App::new();

app.get("/recurso/:id", |req: Request, _res: Response| {
    let id = req.param("id").unwrap_or("");

    if id.is_empty() {
        return Err(HandlerError::with_status(400, "ID inválido"));
    }

    Ok(Response::new().send(format!("ID: {id}")))
})?;
```

- `HandlerError::new(message)`: cria um erro HTTP 500.
- `HandlerError::with_status(status, message)`: cria um erro com estado entre 400 e 599; um código fora desse intervalo é convertido em 500.
- `HandlerError::from_error(error)`: converte um erro Rust externo num `HandlerError`.
- Também existem conversões `From` para `String`, `&str`, `std::io::Error` e `Box<dyn Error + Send + Sync>`.

### Handler global de erros

Configure `set_error_handler` para personalizar a resposta quando um handler ou middleware avançado encaminhar um erro:

```rust
app.set_error_handler(|error: &HandlerError, context| {
    eprintln!(
        "Erro {} em {} {}: {}",
        error.status(),
        context.method(),
        context.path(),
        error
    );

    Response::new()
        .status(error.status())
        .json_error(error.status(), "Não foi possível processar o pedido")
});
```

O `RequestContext` permite consultar o método, caminho, parâmetros de rota, query string e cabeçalhos disponíveis. Evite devolver ao cliente mensagens internas, caminhos locais, credenciais ou detalhes da base de dados. Registe esses detalhes apenas nos logs apropriados.

O router também possui respostas para rotas não encontradas (404) e métodos não permitidos (405). Os templates correspondentes estão em `views/errors/`; a página 500 é usada como fallback de erro interno. Se um template de erro não puder ser carregado, o framework mantém o código HTTP e devolve uma mensagem simples.

Veja `examples/error_handler.rs` para um exemplo executável.

## Logger e mensagens de diagnóstico

RustPress é uma biblioteca Rust, não um ambiente JavaScript: não existe uma função global `console.log()`. Para mensagens pontuais durante o desenvolvimento, pode usar as macros padrão `println!` e `eprintln!`. Para logging estruturado pelo framework, use `Logger`.

Níveis disponíveis:

- `Level::Debug`
- `Level::Info`
- `Level::Warn`
- `Level::Error`

O nível mínimo pode ser definido na aplicação com `app.set_log_level(...)` ou num logger independente com `logger.set_level(...)`.

```rust
use rustpress::{App, Level, Logger};

fn main() {
    let mut app = App::new();
    app.set_log_level(Level::Debug);

    let logger = Logger::new();
    logger.set_level(Level::Debug);

    logger.debug("Informação de diagnóstico");
    logger.info("Aplicação iniciada");
    logger.warn("Atenção: configuração incompleta");
    logger.error("Falha ao executar uma operação");
}
```

O logger inclui timestamp UTC e cores ANSI. DEBUG/INFO são enviados para stdout e WARN/ERROR para stderr. O logger interno do servidor regista o arranque, pedidos HTTP (método, caminho, estado e duração) e erros de ligação. Cada instância tem a sua configuração; clones partilham a configuração dessa instância.

Execute `cargo run --example logger` para ver os níveis em funcionamento.

## Templates e views

O motor de templates é baseado em ficheiros HTML e em `TemplateContext`. Os caminhos são relativos ao directório de trabalho actual do processo. Para que as páginas de erro e o exemplo de views funcionem como documentado, execute a aplicação a partir da raiz do projecto e mantenha a estrutura abaixo.

### Estrutura recomendada

```text
rustpress-web/
├── Cargo.toml
├── README.md
├── assets/
│   └── rustpress-logo.svg
├── examples/
│   ├── basic.rs
│   ├── error_handler.rs
│   ├── logger.rs
│   └── middleware_logging.rs
├── public/
│   └── css/
│       └── style.css
├── src/
│   ├── app.rs
│   ├── handler_error.rs
│   ├── lib.rs
│   ├── logger.rs
│   ├── middleware.rs
│   ├── request.rs
│   ├── response.rs
│   ├── router.rs
│   ├── server.rs
│   └── template.rs
├── tests/
│   ├── advanced_middleware.rs
│   └── static_files.rs
└── views/
    ├── errors/
    │   ├── 404.html
    │   ├── 405.html
    │   └── 500.html
    ├── layouts/
    │   └── main.html
    ├── partials/
    │   └── .gitkeep
    └── index.html
```

**O papel de cada pasta:**

- `src/`: implementação interna da biblioteca.
- `examples/`: aplicações pequenas que demonstram a API.
- `tests/`: testes de integração.
- `public/`: ficheiros estáticos expostos por `App::static_files`.
- `views/`: páginas HTML e templates.
- `views/layouts/`: estruturas comuns que envolvem o conteúdo de cada página.
- `views/partials/`: componentes HTML reutilizáveis incluídos nos templates.
- `views/errors/`: páginas usadas nas respostas de erro do router.

### Ficheiros, layouts e partials

O exemplo `examples/basic.rs` configura a pasta pública e carrega a página inicial:

```rust
app.static_files("/assets", "public");

app.get("/", |_req: Request, res: Response| {
    Ok(res.render_file("views/index.html"))
})?;
```

Com esta configuração, `public/css/style.css` fica disponível em `/assets/css/style.css`. No HTML, use:

```html
<link rel="stylesheet" href="/assets/css/style.css">
```

Para renderizar uma página com layout e partials, o layout deve conter o marcador de conteúdo `{{{conteudo}}}`. Os partials são referenciados pelo nome, sem extensão, e resolvidos dentro do directório indicado:

```html
<!-- views/layouts/main.html -->
<!DOCTYPE html>
<html lang="pt">
<head>
  <meta charset="UTF-8">
  <title>{{titulo}}</title>
</head>
<body>
  {{> cabecalho}}
  <main>{{{conteudo}}}</main>
  {{> rodape}}
</body>
</html>
```

Crie os ficheiros:

- `views/partials/cabecalho.html`
- `views/partials/rodape.html`
- uma página, por exemplo `views/pages/inicio.html` (a pasta `pages` é uma convenção de organização; o caminho é passado explicitamente ao método).

Renderize a página indicando os três caminhos:

```rust
use rustpress::{Request, Response, TemplateContext};
use serde_json::json;

let mut context = TemplateContext::new();
context.set("titulo", json!("Página inicial"));

app.get("/inicio", move |_req: Request, res: Response| {
    Ok(res.render(
        "views/pages/inicio.html",
        "views/layouts/main.html",
        "views/partials",
        &context,
    ))
})?;
```

O método `Response::render` usa renderização estrita: variáveis não definidas e erros de sintaxe fazem a resposta passar para HTTP 500. Para páginas sem layout, use `render_template`; para devolver um ficheiro HTML sem substituição de variáveis, use `render_file`.

### Sintaxe do motor de templates

| Sintaxe | Significado |
|---|---|
| `{{titulo}}` | Substitui uma variável do contexto. |
| `{{#if autenticado}}...{{/if}} ` | Mostra o bloco se o valor for verdadeiro. |
| `{{else}}` | Alternativa dentro de um bloco `if`. |
| `{{#each produtos}}...{{/each}} ` | Repete o bloco para cada elemento de uma lista. |
| `{{> cabecalho}}` | Inclui `views/partials/cabecalho.html` (ou o directório de partials indicado). |

Dentro de um ciclo, as propriedades de cada objecto ficam disponíveis pelo seu nome; para elementos simples, use `{{this}}`. Os valores interpolados são escapados para HTML. Os blocos podem ser aninhados até ao limite interno de profundidade; a validação estrita rejeita blocos mal formados, variáveis inexistentes e inclusões circulares de partials.

Exemplo de contexto:

```rust
use rustpress::TemplateContext;
use serde_json::json;

let mut context = TemplateContext::new();
context.set("titulo", json!("Produtos"));
context.set("autenticado", json!(true));
context.set("produtos", json!([
    { "nome": "Teclado" },
    { "nome": "Rato" }
]));
```

> **Importante:** `TemplateContext` e os métodos de renderização recebem caminhos do sistema de ficheiros. Não são caminhos URL. O directório de trabalho e a existência de layouts/partials influenciam a renderização.

## Ficheiros estáticos

Registe um prefixo URL e a pasta local correspondente:

```rust
app.static_files("/assets", "public");
```

O middleware serve pedidos GET e HEAD, determina o tipo MIME por extensão e rejeita caminhos inválidos ou tentativas de sair do directório público configurado. Não coloque segredos, ficheiros de configuração, backups ou dados privados dentro da pasta pública.

## Exemplos disponíveis

| Exemplo | Objectivo | Comando |
|---|---|---|
| `basic` | Página HTML e ficheiros estáticos | `cargo run --example basic` |
| `error_handler` | Handler global de erros | `cargo run --example error_handler` |
| `logger` | Níveis do logger | `cargo run --example logger` |
| `middleware_logging` | Middleware avançado e respostas JSON | `cargo run --example middleware_logging` |

## Ciclo de uma requisição

1. O servidor Tokio aceita uma ligação TCP e o Hyper processa o pedido HTTP.
2. O router identifica o método e procura a rota compatível com o caminho.
3. Os parâmetros dinâmicos são extraídos e disponibilizados em `Request::param`.
4. Os middlewares registados são executados.
5. O handler constrói uma resposta ou devolve um `HandlerError`.
6. Erros encaminhados são tratados pelo handler global, quando configurado.
7. A resposta RustPress é convertida para a resposta Hyper e enviada ao cliente.

## Testes

Execute a suite completa:

```bash
cargo test
```

Para validar também os exemplos:

```bash
cargo check --examples
```

## Limitações conhecidas e funcionalidades futuras

As funcionalidades abaixo **não devem ser consideradas implementadas** só por existirem rotas HTTP ou middleware:

- leitura e descodificação do corpo dos pedidos;
- parsing de formulários HTML (`application/x-www-form-urlencoded`);
- parsing de `multipart/form-data` e upload de ficheiros;
- sessão, cookies e autenticação integrados;
- acesso integrado a bases de dados;
- rotas agrupadas e routers modulares;
- API completa de respostas como redireccionamentos e envio de ficheiros para download;
- suporte a WebSocket.

Estas áreas podem ser desenvolvidas futuramente, com APIs e testes próprios. O roadmap deve ser actualizado à medida que cada funcionalidade for efectivamente implementada.

## Documentação adicional

- [Arquitectura do RustPress](docs/ARCHITECTURE.md)
- [Guia de contribuição](docs/CONTRIBUTING.md)
- [Licença MIT](LICENSE)

## Contribuir

Contribuições, sugestões e relatórios de erros são bem-vindos. Antes de abrir um pull request:

```bash
cargo fmt --check
cargo test
cargo check --examples
```

Consulte [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) para o fluxo de trabalho.

## Licença

Este projecto está licenciado sob a licença MIT. Consulte [LICENSE](LICENSE).

## Autor e repositório

- **Autor:** Fabrício Michael
- **Repositório:** https://github.com/fabriciomichael/rustpress-web
