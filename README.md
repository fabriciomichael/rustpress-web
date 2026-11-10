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
- **Rotas HTTP:** GET, POST, PUT, PATCH e DELETE.
- **WebSocket:** comunicação bidireccional em tempo real, com mensagens de texto e binárias, parâmetros dinâmicos nas rotas, encerramento de ligações e controlo do handshake através de middleware global. Registo através de `App::ws`.
- **Grupos de rotas:** agrupamento de rotas com prefixos, suporte a grupos aninhados e middleware específico por grupo.
- **Parâmetros dinâmicos:** padrões como `/utilizadores/:id`.
- **Query strings:** consulta de parâmetros como `?pagina=2&limite=10`.
- **Request:** leitura do corpo, texto, JSON, formulários URL-encoded, headers, cookies, parâmetros e sessões.
- **Response:** respostas em texto, HTML, JSON e conteúdo binário, headers, códigos de estado, cookies e redireccionamentos.
- **Middleware:** middleware simples e cadeia avançada com continuação, resposta imediata ou erro.
- **Autenticação e permissões:** middleware de autenticação e controlo de funções ou papéis dos utilizadores.
- **Protecção CSRF:** validação de tokens através de headers ou formulários.
- **Sessões:** gestão de sessões, armazenamento em memória, regeneração e destruição de sessões.
- **Tratamento global de erros:** handler global e páginas de erro 404, 405 e 500.
- **Segurança de palavras-passe:** geração e verificação de hashes com Argon2id.
- **Upload de ficheiros:** recepção de multipart/form-data, validação de limites e armazenamento com nomes gerados pelo framework.
- **Ficheiros estáticos:** publicação de CSS, JavaScript, imagens e outros ficheiros a partir de uma pasta configurada.
- **Templates HTML:** variáveis, condições, ciclos, layouts e partials, com validação rigorosa e escape de valores.
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


## Criar um projecto com RustPress CLI

O RustPress CLI é a ferramenta oficial para criar a estrutura inicial de aplicações que utilizam o RustPress Web.

### Instalação

Instala o CLI através do Cargo:

```bash
cargo install rustpress-cli
```

### Criar um projecto

Depois da instalação, cria um novo projecto:

```bash
rustpress new minha_app
```

### Executar a aplicação

Entra no directório criado e inicia a aplicação:

```bash
cd minha_app
cargo run
```

Por padrão, o servidor de demonstração é iniciado em:

```text
http://127.0.0.1:3000
```

### Comandos disponíveis

Para consultar os comandos e opções do CLI:

```bash
rustpress --help
```

Para consultar a versão instalada:

```bash
rustpress --version
```

O projecto gerado utiliza a dependência `rustpress-web = "0.2.0"` e o runtime Tokio.

Código-fonte do RustPress CLI: https://github.com/fabriciomichael/rustpress-cli

## Estrutura do projecto

O RustPress CLI gera uma estrutura inicial semelhante à seguinte:

```text
minha_app/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   └── web.rs
│   ├── controllers/
│   │   ├── mod.rs
│   │   └── home_controller.rs
│   ├── models/
│   │   └── mod.rs
│   ├── middleware/
│   │   └── mod.rs
│   └── config/
│       └── mod.rs
├── views/
│   ├── layouts/
│   ├── pages/
│   │   └── home.html
│   └── partials/
├── public/
│   ├── css/
│   ├── js/
│   └── images/
└── tests/
```




O servidor de demonstração fica disponível em `http://127.0.0.1:3000`.

## Arranque rápido

O exemplo seguinte regista rotas, lê um parâmetro dinâmico e responde a um pedido POST. Os handlers GET e POST actuais devolvem `Result<Response, HandlerError>`, por isso é possível propagar erros com `?`.

```rust
use rustpress_web::{App, Request, Response};

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

### API do Request

O tipo `Request` disponibiliza métodos para consultar os dados da requisição HTTP:

- `method()`: devolve o método HTTP utilizado.
- `path()`: devolve o caminho da URL, sem a query string.
- `header(name)`: consulta um cabeçalho HTTP.
- `param(name)`: obtém o valor de um parâmetro dinâmico da rota.
- `query(name)`: obtém o valor de um parâmetro da query string.
- `cookies()`: devolve a lista de cookies recebidos.
- `cookie(name)`: procura um cookie pelo nome.
- `body()`: disponibiliza o corpo da requisição em bytes.
- `text()`: interpreta o corpo como texto UTF-8.
- `json<T>()`: converte o corpo JSON numa estrutura Rust que implemente `DeserializeOwned`.
- `form()`: descodifica campos de formulários URL-encoded ou multipart/form-data, preservando a ordem e os nomes repetidos.
- `files()`: devolve os ficheiros recebidos através de multipart/form-data.
- `file(field_name)`: procura o primeiro ficheiro associado ao nome do campo.
- `session()`: disponibiliza a sessão associada à requisição, quando activada.
- `regenerate_session()`: regenera o identificador da sessão actual.
- `context()`: cria uma cópia dos metadados da requisição para o tratamento de erros.



### API do Response

O tipo `Response` disponibiliza métodos para construir respostas HTTP:

- `Response::new()`: cria uma resposta com estado HTTP 200.
- `.status(code)`: define o código de estado HTTP.
- `.status_code()`: consulta o código de estado actual.
- `.send(body)`: envia texto simples.
- `.html(body)`: envia conteúdo HTML.
- `.json(body)`: envia conteúdo JSON em formato textual.
- `Response::error(code, message)`: cria uma resposta de erro em texto simples.
- `Response::json_error(code, message)`: cria uma resposta de erro em JSON.
- `.header(name, value)`: adiciona ou substitui um cabeçalho HTTP.
- `.send_bytes(bytes)`: envia conteúdo binário.
- `.render_file(path)`: carrega um ficheiro HTML sem processar variáveis.
- `.render_template(path, context)`: carrega e processa um template com contexto.
- `.render(page, layout, partials_dir, context)`: renderiza uma página com layout e partials, usando validação rigorosa.
- `Response::error_page(...)`: cria uma página de erro personalizada.
- `.cookie(cookie, config)`: adiciona um cookie à resposta.
- `.remove_cookie(name, config)`: envia um cabeçalho para remover um cookie.
- `Response::redirect(path)`: cria um redireccionamento HTTP 302.
- `Response::redirect_with_status(code, path)`: cria um redireccionamento com os estados 301, 302, 303, 307 ou 308.

**Nota sobre JSON:** o método `.json(...)` recebe conteúdo em formato textual. Para converter estruturas Rust em JSON, pode utilizar `serde_json::json!(...).to_string()`.

**Nota sobre cookies:** os métodos `cookie()` e `remove_cookie()` recebem uma configuração `SessionConfig` e devolvem um `Result`, permitindo detectar erros na criação dos cabeçalhos.

## Comunicação WebSocket

O RustPress suporta comunicação bidireccional em tempo real através do protocolo **WebSocket**. Esta funcionalidade permite estabelecer ligações persistentes entre clientes e servidores, possibilitando a troca de mensagens sem necessidade de efectuar novos pedidos HTTP.

As rotas WebSocket são registadas através do método `App::ws()`.

### Criar uma rota WebSocket

O exemplo seguinte implementa um servidor echo: todas as mensagens de texto recebidas são devolvidas ao cliente.

```rust
use rustpress_web::{App, Message, Request, WebSocket};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    app.ws("/ws", |_request: Request, mut websocket: WebSocket| async move {
        loop {
            match websocket.recv().await {
                Ok(Some(Message::Text(text))) => {
                    if websocket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                Ok(Some(Message::Close)) | Ok(None) => {
                    break;
                }
                Ok(Some(_)) => {}
                Err(error) => {
                    eprintln!("Erro WebSocket: {error}");
                    break;
                }
            }
        }
    })?;

    app.listen("127.0.0.1:3000").await?;

    Ok(())
}
```

Guarde o código em `examples/websocket.rs` e execute-o com:

```bash
cargo run --example websocket
```

O servidor ficará disponível em `127.0.0.1:3000`, aguardando ligações WebSocket no caminho `/ws`.

### API WebSocket

O RustPress disponibiliza os seguintes tipos e métodos:

| Elemento | Descrição |
|---|---|
| `App::ws(path, handler)` | Regista uma rota WebSocket. |
| `WebSocket::send(message)` | Envia uma mensagem. |
| `WebSocket::recv()` | Recebe a próxima mensagem. |
| `WebSocket::close()` | Encerra a ligação de forma controlada. |
| `Message::Text(String)` | Mensagem de texto UTF-8. |
| `Message::Binary(Vec<u8>)` | Mensagem binária. |
| `Message::Ping(Vec<u8>)` | Mensagem de controlo Ping. |
| `Message::Pong(Vec<u8>)` | Mensagem de controlo Pong. |
| `Message::Close` | Mensagem de encerramento. |
| `WebSocketError` | Tipo de erro das operações WebSocket. |

O método `recv()` devolve `Result<Option<Message>, WebSocketError>`. Quando recebe uma mensagem de encerramento, devolve `Ok(Some(Message::Close))`. Quando o fluxo termina sem outra mensagem, devolve `Ok(None)`.

### Funcionalidades suportadas

- Handshake e actualização de pedidos HTTP para WebSocket.
- Comunicação através de mensagens de texto e binárias.
- Mensagens de controlo Ping e Pong.
- Rotas com parâmetros dinâmicos, como `/ws/:id`.
- Execução dos middlewares globais antes da aceitação da ligação.
- Encerramento controlado e detecção de desconexões.

**Nota:** os middlewares globais podem rejeitar o handshake antes de a ligação WebSocket ser estabelecida.

## Segurança de palavras-passe

O RustPress disponibiliza o módulo `security` para proteger palavras-passe através do algoritmo **Argon2id**.

A API permite gerar hashes seguros e verificar palavras-passe sem armazenar os valores originais.

### Gerar um hash

```rust
use rustpress_web::security::Password;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let password = "MinhaSenhaSegura123";

    let hash = Password::hash(password)?;

    println!("Hash: {hash}");

    Ok(())
}
```

### Verificar uma palavra-passe

```rust
use rustpress_web::security::Password;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let password = "MinhaSenhaSegura123";

    let hash = Password::hash(password)?;

    if Password::verify(password, &hash)? {
        println!("Palavra-passe correcta!");
    } else {
        println!("Palavra-passe incorrecta!");
    }

    Ok(())
}
```

### Configuração personalizada

A configuração padrão utiliza Argon2id com os seguintes parâmetros:

- Memória: 19 456 KiB (aproximadamente 19 MiB);
- Iterações: 2;
- Paralelismo: 1;
- Tamanho do hash: 32 bytes.

É possível fornecer uma configuração personalizada através de `PasswordConfig` e do método `Password::hash_with_config()`. Valores inferiores aos padrões podem reduzir a resistência do hash a ataques de força bruta. Ajuste estes parâmetros tendo em conta a segurança e os recursos disponíveis no servidor.

```rust
use rustpress_web::security::{Password, PasswordConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PasswordConfig {
        memory_cost: 19_456,
        time_cost: 2,
        parallelism: 1,
        output_length: 32,
    };

    let hash = Password::hash_with_config("MinhaSenhaSegura123", config)?;

    println!("Hash: {hash}");

    Ok(())
}
```
## Upload de ficheiros

O RustPress permite receber ficheiros através de pedidos HTTP com conteúdo `multipart/form-data`. A funcionalidade tem de ser activada explicitamente através de `App::use_uploads()`.

### Configurar o armazenamento

```rust
use rustpress_web::{App, upload::UploadConfig};

let mut app = App::new();

app.use_uploads(
    UploadConfig::new("storage/uploads")
)?;
```

A configuração permite definir limites para proteger a aplicação:

| Método | Descrição | Padrão |
|---|---|---:|
| `with_max_total_size(size)` | Tamanho máximo total do pedido | 25 MiB |
| `with_max_file_size(size)` | Tamanho máximo de cada ficheiro | 10 MiB |
| `with_max_files(count)` | Número máximo de ficheiros | 5 |
| `with_max_fields(count)` | Número máximo de campos de texto | 32 |
| `with_max_field_size(size)` | Tamanho máximo de cada campo de texto | 64 KiB |

Os tamanhos são definidos em bytes. Os limites personalizados são validados antes de serem aceites.

### Receber um ficheiro

```rust
use rustpress_web::{App, Request, Response};
use rustpress_web::upload::UploadConfig;

let mut app = App::new();

app.use_uploads(
    UploadConfig::new("storage/uploads")
)?;

app.post("/upload", |req: Request, res: Response| {
    if let Some(file) = req.file("avatar") {
        println!("Nome original: {:?}", file.original_name());
        println!("Nome armazenado: {}", file.stored_name());
        println!("Caminho: {:?}", file.path());
        println!("Tamanho: {} bytes", file.size());
        println!("Tipo de conteúdo: {:?}", file.content_type());
    }

    Ok(res.send("Pedido recebido"))
})?;
```

O formulário HTML correspondente pode ser:

```html
<form action="/upload" method="POST" enctype="multipart/form-data">
    <input type="file" name="avatar">
    <button type="submit">Enviar ficheiro</button>
</form>
```

### Consultar ficheiros e campos

O `Request` disponibiliza:

- `file("avatar")`: procura um ficheiro pelo nome do campo.
- `files()`: devolve a lista de ficheiros recebidos.
- `form()`: devolve os campos de texto do formulário.

Cada `UploadedFile` disponibiliza o nome do campo, o nome original, o nome armazenado, o caminho, o tipo de conteúdo e o tamanho.

### Segurança e limitações

- Os ficheiros são armazenados com nomes únicos gerados pelo framework; o nome fornecido pelo cliente não é utilizado como caminho de armazenamento.
- O nome original é disponibilizado apenas como metadado e não deve ser considerado confiável.
- Os limites de tamanho e quantidade são aplicados durante o processamento.
- Se o processamento falhar ou um middleware rejeitar o pedido, os ficheiros temporários ou já guardados durante esse processamento são removidos.
- O processamento actual recebe o corpo multipart em memória antes de analisar os campos e guardar os ficheiros. Portanto, ainda não é uma implementação de upload em streaming.
## Sessões e autenticação

O RustPress disponibiliza sessões HTTP através de `App::use_sessions()`. Por padrão, os dados são guardados em memória (`MemoryStore`) e identificados por um cookie chamado `rustpress_session`.

### Activar sessões

Active a gestão de sessões antes de iniciar o servidor:

```rust
use rustpress_web::{App, session::SessionConfig};

let mut app = App::new();
app.use_sessions(SessionConfig::default())?;
```

Pode personalizar o cookie e o tempo de vida da sessão:

```rust
use rustpress_web::session::{SameSite, SessionConfig};

let config = SessionConfig {
    cookie_name: "minha_app_session".to_string(),
    max_age: 60 * 60, // uma hora, em segundos
    http_only: true,
    secure: true, // produção com HTTPS
    same_site: SameSite::Lax,
    ..SessionConfig::default()
};

app.use_sessions(config)?;
```

Em desenvolvimento local com HTTP, mantenha `secure: false`. Em produção, use HTTPS e active `secure: true`. O armazenamento padrão é em memória: as sessões não sobrevivem ao reinício do processo nem são partilhadas entre várias instâncias da aplicação.

### Criar e guardar dados na sessão

Quando as sessões estão activadas, cada pedido recebe uma sessão. Insira ou actualize os dados através de `Session::insert()`:

```rust
use rustpress_web::{HandlerError, Request, Response};

app.post("/preferencias", |req: Request, _res: Response| {
    let session = req.session()
        .ok_or_else(|| HandlerError::new("Sessões não estão activadas."))?;

    let mut session = session.lock()
        .map_err(|_| HandlerError::new("Não foi possível aceder à sessão."))?;

    session.insert("tema", "escuro")
        .map_err(HandlerError::from_error)?;

    session.insert("notificacoes", true)
        .map_err(HandlerError::from_error)?;

    Ok(Response::redirect_with_status(303, "/preferencias"))
})?;
```

O RustPress guarda automaticamente a sessão modificada no final do pedido e envia o cookie correspondente na resposta. Não é necessário chamar um método `save()` no handler.

### Recuperar dados e o ID do utilizador

Os dados são recuperados com `Session::get::<T>()`. A aplicação deve guardar explicitamente o identificador do utilizador autenticado, por exemplo na chave `user_id`:

```rust
use rustpress_web::{HandlerError, Request, Response};

app.get("/minha-conta", |req: Request, _res: Response| {
    let session = req.session()
        .ok_or_else(|| HandlerError::new("Sessões não estão activadas."))?;

    let session = session.lock()
        .map_err(|_| HandlerError::new("Não foi possível aceder à sessão."))?;

    let Some(user_id) = session.get::<u64>("user_id") else {
        return Ok(Response::redirect("/login"));
    };

    Ok(Response::json(
        serde_json::json!({ "user_id": user_id }).to_string()
    ))
})?;
```

**Importante:** `user_id` é um dado da aplicação guardado durante o login. Não é o mesmo que o identificador técnico da sessão. Este último pode ser consultado com `session.id()`, mas deve ser tratado como um segredo: não o exponha em URLs, respostas, logs públicos ou código do lado do cliente.

### Exemplo de login com sessão

O exemplo seguinte demonstra o fluxo completo. As credenciais fixas servem apenas para ilustrar a integração; numa aplicação real, consulte o utilizador na base de dados e verifique a palavra-passe com `security::Password::verify()`.

```rust
use rustpress_web::{
    App, HandlerError, Request, Response,
    session::SessionConfig,
};

let mut app = App::new();
app.use_sessions(SessionConfig::default())?;

app.post("/login", |req: Request, _res: Response| {
    let form = req.form();

    let username = form.iter()
        .find(|(key, _)| key == "username")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();

    let password = form.iter()
        .find(|(key, _)| key == "password")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();

    // Demonstração: substitua pela consulta à base de dados
    // e por Password::verify(&password, &hash_guardado).
    if username != "demo" || password != "demo123" {
        return Ok(Response::redirect("/login?erro=credenciais"));
    }

    // Renova o ID depois de autenticar, reduzindo o risco de session fixation.
    let regenerated = req.regenerate_session()
        .map_err(HandlerError::from_error)?;

    if !regenerated {
        return Err(HandlerError::new("Não foi possível regenerar a sessão."));
    }

    let session = req.session()
        .ok_or_else(|| HandlerError::new("Sessão não disponível."))?;

    let mut session = session.lock()
        .map_err(|_| HandlerError::new("Não foi possível aceder à sessão."))?;

    session.insert("user_id", 42_u64)
        .map_err(HandlerError::from_error)?;

    session.insert("username", username)
        .map_err(HandlerError::from_error)?;

    Ok(Response::redirect_with_status(303, "/painel"))
})?;
```

Formulário HTML correspondente:

```html
<form action="/login" method="POST">
    <label>
        Utilizador
        <input type="text" name="username" required>
    </label>

    <label>
        Palavra-passe
        <input type="password" name="password" required>
    </label>

    <button type="submit">Entrar</button>
</form>
```

### Proteger uma rota

Verifique a presença de `user_id` antes de devolver conteúdo privado:

```rust
app.get("/painel", |req: Request, _res: Response| {
    let session = req.session()
        .ok_or_else(|| HandlerError::new("Sessões não estão activadas."))?;

    let session = session.lock()
        .map_err(|_| HandlerError::new("Não foi possível aceder à sessão."))?;

    let Some(user_id) = session.get::<u64>("user_id") else {
        return Ok(Response::redirect("/login"));
    };

    Ok(Response::html(format!(
        "<h1>Painel do utilizador {user_id}</h1>"
    )))
})?;
```

Para aplicações maiores, também pode utilizar `AuthMiddleware` e `RoleMiddleware`. O middleware de autenticação verifica a chave `user_id`; o middleware de papéis consulta a chave `role`.

### Terminar sessão (logout)

Chame `Session::destroy()` para destruir a sessão actual:

```rust
app.post("/logout", |req: Request, _res: Response| {
    let session = req.session()
        .ok_or_else(|| HandlerError::new("Sessões não estão activadas."))?;

    let mut session = session.lock()
        .map_err(|_| HandlerError::new("Não foi possível aceder à sessão."))?;

    session.destroy();

    Ok(Response::redirect_with_status(303, "/login"))
})?;
```

Ao detectar uma sessão destruída, o RustPress remove-a do armazenamento, revoga o identificador e envia um cookie de remoção ao navegador. Não é necessário chamar manualmente `Response::remove_cookie()` para terminar uma sessão gerida pelo framework.

### Remover dados sem terminar a sessão

Use `Session::remove()` para apagar apenas uma chave, ou `Session::clear()` para limpar todos os dados sem destruir o identificador:

```rust
// Remover apenas o ID do utilizador:
session.remove("user_id");

// Remover todos os dados, mantendo a sessão:
session.clear();
```

Use `destroy()` para logout. Use `clear()` apenas quando pretende manter a sessão activa, mas sem os dados actuais.

### Regenerar o identificador da sessão

Depois de autenticar o utilizador, regenere o identificador para reduzir o risco de session fixation:

```rust
let regenerated = req.regenerate_session()
    .map_err(HandlerError::from_error)?;

if !regenerated {
    return Err(HandlerError::new("Gestão de sessões não está activada."));
}
```

A regeneração cria um novo UUID, preserva os dados existentes e revoga o identificador anterior. O novo cookie é enviado quando a sessão modificada é finalizada.

## Redireccionamentos HTTP

O RustPress disponibiliza métodos estáticos em `Response` para redireccionar o cliente para outro caminho ou URL.

### Redireccionamento simples

```rust
app.get("/antiga-pagina", |_req: Request, _res: Response| {
    Ok(Response::redirect("/nova-pagina"))
})?;
```

`Response::redirect(path)` cria uma resposta HTTP **302 Found**, adequada para um redireccionamento temporário.

### Escolher o código HTTP

Use `Response::redirect_with_status(code, path)` quando precisar de definir explicitamente o estado:

```rust
// Após um POST, encaminha o navegador para uma página GET.
Ok(Response::redirect_with_status(303, "/resultado"))

// Temporário e preserva o método HTTP e o corpo.
Ok(Response::redirect_with_status(307, "/novo-endpoint"))

// Redireccionamento permanente.
Ok(Response::redirect_with_status(301, "/novo-endereco"))
```

Códigos aceites:

| Código | Significado | Comportamento |
|---:|---|---|
| 301 | Moved Permanently | Redireccionamento permanente. |
| 302 | Found | Redireccionamento temporário; é o padrão de `Response::redirect()`. |
| 303 | See Other | Indica ao cliente que deve consultar o destino com GET; útil após POST/PUT/DELETE (Post/Redirect/Get). |
| 307 | Temporary Redirect | Temporário e preserva o método HTTP e o corpo do pedido. |
| 308 | Permanent Redirect | Permanente e preserva o método HTTP e o corpo do pedido. |

O destino pode ser um caminho local, como `/login`, ou uma URI absoluta válida. Não construa destinos a partir de dados não confiáveis do utilizador: valide-os e prefira caminhos locais conhecidos para evitar redireccionamentos abertos (*open redirects*).

## Rotas em grupo

O RustPress permite organizar rotas relacionadas em grupos através de `App::group()`. Um grupo aplica um prefixo comum aos caminhos das rotas e permite associar middlewares a todas as rotas desse grupo.

Esta funcionalidade é útil para estruturar APIs, separar áreas públicas e privadas e organizar módulos como administração, utilizadores e produtos.

### Criar um grupo de rotas

O método `group()` recebe dois argumentos:

- `prefix`: prefixo comum das rotas do grupo.
- `configure`: closure que recebe um `RouteGroup` e regista as rotas.

Exemplo:

```rust
use rustpress_web::{App, Request, Response};

let mut app = App::new();

app.group("/utilizadores", |users| {
    users.get("/", |_req: Request, _res: Response| {
        Ok(Response::json(r#"{"mensagem":"Lista de utilizadores"}"#))
    })?;

    users.get("/:id", |req: Request, _res: Response| {
        let id = req.param("id").unwrap_or("desconhecido");

        Ok(Response::json(
            format!(r#"{{"utilizador":"{}"}}"#, id)
        ))
    })?;

    users.post("/", |_req: Request, _res: Response| {
        Ok(Response::json(r#"{"mensagem":"Utilizador criado"}"#))
    })?;

    Ok(())
})?;
```

As rotas registadas ficam disponíveis nos seguintes caminhos:

| Método | Caminho final |
|---|---|
| GET | `/utilizadores/` |
| GET | `/utilizadores/:id` |
| POST | `/utilizadores/` |

O prefixo é combinado automaticamente com o caminho de cada rota. Pode escrever os caminhos das rotas com ou sem a barra inicial; o RustPress normaliza a junção.

### Métodos HTTP disponíveis

Um `RouteGroup` suporta os métodos GET, POST, PUT, PATCH e DELETE:

```rust
app.group("/api/produtos", |products| {
    products.get("/", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"GET"}"#))
    })?;

    products.post("/", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"POST"}"#))
    })?;

    products.put("/:id", |_req, _res| {
        Response::json(r#"{"metodo":"PUT"}"#)
    })?;

    products.patch("/:id", |_req, _res| {
        Response::json(r#"{"metodo":"PATCH"}"#)
    })?;

    products.delete("/:id", |_req, _res| {
        Response::json(r#"{"metodo":"DELETE"}"#)
    })?;

    Ok(())
})?;
```

Os handlers GET e POST recebem uma closure que devolve `Result<Response, HandlerError>`. Os métodos PUT, PATCH e DELETE também disponibilizam variantes `put_fallible()`, `patch_fallible()` e `delete_fallible()` para handlers que precisam de devolver erros:

```rust
use rustpress_web::HandlerError;

app.group("/api", |api| {
    api.put_fallible("/recurso/:id", |_req, _res| {
        Err(HandlerError::with_status(422, "Dados inválidos"))
    })?;

    api.patch_fallible("/recurso/:id", |_req, _res| {
        Err(HandlerError::with_status(422, "Não foi possível actualizar"))
    })?;

    api.delete_fallible("/recurso/:id", |_req, _res| {
        Err(HandlerError::with_status(404, "Recurso não encontrado"))
    })?;

    Ok(())
})?;
```

### Aplicar middleware a um grupo

Use `use_middleware()` para associar um middleware simples a todas as rotas do grupo. Se devolver `Some(Response)`, a execução da rota é interrompida; se devolver `None`, o pedido continua:

```rust
app.group("/admin", |admin| {
    admin.use_middleware(|req| {
        if req.header("x-admin-token")
            .and_then(|value| value.to_str().ok())
            == Some("token-de-exemplo")
        {
            None
        } else {
            Some(Response::new()
                .status(401)
                .send("Acesso não autorizado"))
        }
    });

    admin.get("/painel", |_req, _res| {
        Ok(Response::html("<h1>Painel administrativo</h1>"))
    })?;

    admin.get("/relatorios", |_req, _res| {
        Ok(Response::html("<h1>Relatórios</h1>"))
    })?;

    Ok(())
})?;
```

O exemplo utiliza um token fixo apenas para demonstrar o funcionamento do middleware. Numa aplicação real, utilize autenticação segura, sessões ou os middlewares de autenticação disponibilizados pelo RustPress.

Para controlo mais avançado, use `use_middleware_advanced()`. Este método permite continuar a cadeia, responder directamente ou devolver um erro através de `MiddlewareResult`:

```rust
use rustpress_web::{MiddlewareResult, Response};

app.group("/api", |api| {
    api.use_middleware_advanced(|req| {
        if req.header("x-api-key").is_some() {
            MiddlewareResult::Next
        } else {
            MiddlewareResult::Respond(
                Response::new()
                    .status(401)
                    .send("Chave da API em falta")
            )
        }
    });

    api.get("/dados", |_req, _res| {
        Ok(Response::json(r#"{"estado":"autorizado"}"#))
    })?;

    Ok(())
})?;
```

### Grupos aninhados

Pode criar grupos dentro de outros grupos. Os prefixos são combinados e os middlewares são herdados na ordem de hierarquia:

```rust
app.group("/admin", |admin| {
    admin.use_middleware_advanced(|_req| {
        println!("Middleware do grupo admin");
        MiddlewareResult::Next
    });

    admin.group("/utilizadores", |users| {
        users.use_middleware_advanced(|_req| {
            println!("Middleware do grupo utilizadores");
            MiddlewareResult::Next
        });

        users.get("/", |_req, _res| {
            Ok(Response::json(r#"{"area":"admin/utilizadores"}"#))
        })?;

        users.get("/:id", |req, _res| {
            let id = req.param("id").unwrap_or("desconhecido");

            Ok(Response::json(
                format!(r#"{{"id":"{}"}}"#, id)
            ))
        })?;

        Ok(())
    })?;

    Ok(())
})?;
```

A rota de listagem do exemplo fica disponível em `GET /admin/utilizadores/`, e a rota individual em `GET /admin/utilizadores/:id`.

A ordem de execução dos middlewares é:

1. Middlewares globais registados na aplicação.
2. Middlewares do grupo exterior (`/admin`).
3. Middlewares do grupo interior (`/admin/utilizadores`).
4. Handler da rota.

Os middlewares de um grupo são aplicados a todas as rotas desse grupo, mesmo que sejam declarados depois de algumas rotas dentro da closure.

### Tratamento de erros e validação dos grupos

O método `group()` devolve `Result<(), String>`; por isso, use `?` ou trate explicitamente o erro. O RustPress valida os caminhos e rejeita conflitos ou rotas duplicadas.

A configuração do grupo é atómica: se a closure devolver um erro, ou se houver conflito ao registar as rotas, nenhuma das rotas desse grupo é adicionada à aplicação. Isto evita deixar a aplicação parcialmente configurada.

```rust
let result = app.group("/admin", |admin| {
    admin.get("/painel", |_req, _res| {
        Ok(Response::html("<h1>Painel</h1>"))
    })?;

    // Simula uma falha durante a configuração.
    Err("Falha ao configurar o grupo".to_string())
});

if let Err(error) = result {
    eprintln!("Não foi possível registar o grupo: {error}");
}
```

**Nota:** os grupos organizam rotas e middlewares durante a configuração da aplicação. Não criam servidores separados nem alteram, por si só, as permissões de acesso. Para proteger rotas, associe middlewares de autenticação/autorização adequados.

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
use rustpress_web::{App, MiddlewareResult, Request, Response};

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
use rustpress_web::{App, HandlerError, Request, Response};

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
use rustpress_web::{App, Level, Logger};

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
    │   ├── header.html
    │   └── footer.html
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
use rustpress_web::{Request, Response, TemplateContext};
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
use rustpress_web::TemplateContext;
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

## Documentação da API (rustdoc)

A API pública é documentada directamente nos módulos e tipos de `src/` através de comentários Rustdoc (`///` e `//!`). A documentação inclui descrições, exemplos executáveis e notas sobre limitações conhecidas.

Gerar a documentação HTML localmente:

```bash
cargo doc --no-deps --open
```

Executar também os exemplos incluídos nos comentários da documentação:

```bash
cargo test --doc
```

Verificar a documentação e os exemplos antes de preparar uma publicação:

```bash
cargo fmt --check
cargo check
cargo test
cargo test --doc
cargo package --list
cargo package
```

O atributo `#![warn(missing_docs)]` está activo na crate para sinalizar itens públicos sem documentação. Antes da publicação, corrija todos os avisos e confirme que os exemplos de rustdoc compilam.

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

Apesar das funcionalidades já implementadas, algumas áreas ainda podem ser desenvolvidas:

- **Integração com bases de dados:** suporte integrado para acesso e gestão de bases de dados.

Estas funcionalidades fazem parte das possíveis evoluções do RustPress Web e deverão ser acompanhadas de APIs e testes próprios.

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
