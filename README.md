<p align="center">
  <img src="assets/rustpress-logo.svg" alt="RustPress — HTTP framework for Rust" width="520">
</p>

<p align="center">
  <strong>A lightweight HTTP framework for Rust, inspired by the simplicity of Express.js.</strong>
</p>

<p align="center">
  <a href="https://github.com/fabriciomichael/rustpress-web">GitHub</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/CONTRIBUTING.md">Contributing</a>
</p>

---

## About the Project

**RustPress** is an asynchronous HTTP framework for Rust, built on **Hyper** and **Tokio**. Its goal is to provide a straightforward API for building web servers and APIs, with routing, middleware, HTTP responses, static files, templates, and centralized error handling.

The project aims to combine a familiar interface for developers who know Express.js with Rust's principles of safety, performance, and type safety.

> RustPress is under active development. Check the API and existing examples in this repository before assuming that a feature available in other frameworks is already supported.

## Current Features

- **Asynchronous HTTP server:** Hyper 1 and Tokio.
- **HTTP routing:** GET, POST, PUT, PATCH, and DELETE.
- **WebSocket:** real-time bidirectional communication with text and binary messages, dynamic route parameters, connection closing, and handshake control through global middleware. Registered using `App::ws`.
- **Route groups:** route grouping with prefixes, nested groups, and group-specific middleware.
- **Dynamic parameters:** patterns such as `/users/:id`.
- **Query strings:** query parameters such as `?page=2&limit=10`.
- **Request:** access to the body, text, JSON, URL-encoded forms, headers, cookies, parameters, and sessions.
- **Response:** text, HTML, JSON, and binary responses, headers, status codes, cookies, and redirects.
- **Middleware:** simple middleware and advanced middleware chains with continuation, immediate responses, or errors.
- **Authentication and permissions:** middleware for authentication and role-based access control.
- **CSRF protection:** token validation through headers or forms.
- **Sessions:** session management, in-memory storage, session regeneration, and destruction.
- **Global error handling:** global error handler and 404, 405, and 500 error pages.
- **Password security:** password hashing and verification using Argon2id.
- **File uploads:** multipart/form-data handling, size-limit validation, and storage using framework-generated filenames.
- **Static files:** serving CSS, JavaScript, images, and other files from a configured directory.
- **HTML templates:** variables, conditionals, loops, layouts, and partials, with strict validation and value escaping.
- **Logger:** DEBUG, INFO, WARN, and ERROR levels, with colored output and server request logging.
- **Tests:** unit and integration tests for existing features.

## Requirements

- Stable Rust with support for the **2024 edition**.
- Cargo (installed with Rust).
- An operating system supported by Tokio and Hyper.

## Building and Running the Repository

Clone the project:

```bash
git clone https://github.com/fabriciomichael/rustpress-web.git
cd rustpress-web
```

Build the project and run the tests:

```bash
cargo build
cargo test
```

Run an example:

```bash
cargo run --example basic
```

## Creating a Project with RustPress CLI

RustPress CLI is the official tool for generating the initial structure of applications built with RustPress Web.

### Installation

Install the CLI using Cargo:

```bash
cargo install rustpress-cli
```

### Creating a Project

After installation, create a new project:

```bash
rustpress new minha_app
```

### Running the Application

Navigate to the generated directory and start the application:

```bash
cd minha_app
cargo run
```

By default, the example server starts at:

```text
http://127.0.0.1:3000
```

### Available Commands

To view the CLI commands and options:

```bash
rustpress --help
```

To check the installed version:

```bash
rustpress --version
```

The generated project uses the `rustpress-web = "0.3.1"` dependency and the Tokio runtime.

RustPress CLI source code: https://github.com/fabriciomichael/rustpress-cli

## Project Structure

RustPress CLI generates an initial project structure like the following:

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

The example server is available at `http://127.0.0.1:3000`.

## Quick Start

The following example registers routes, reads a dynamic parameter, and handles a POST request. The current GET and POST handlers return `Result<Response, HandlerError>`, so errors can be propagated using `?`.

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

Save the code, for example, in `examples/basic_api.rs`, and run it with:

```bash
cargo run --example basic_api
```

## HTTP Routes

Routes are registered through an `App` instance. The available methods are:

| Method | Registration | Fallible Handler |
|---|---|---|
| GET | `app.get(path, handler)` | Yes |
| POST | `app.post(path, handler)` | Yes |
| PUT | `app.put(path, handler)` | Yes |
| PATCH | `app.patch(path, handler)` | Yes |
| DELETE | `app.delete(path, handler)` | Yes |

Route registration returns `Result<(), String>`; use `?` or handle the error to detect invalid route patterns.

### Route Parameters

```rust
app.get("/produtos/:id", |req: Request, res: Response| {
    let id = req.param("id").unwrap_or("0");
    Ok(res.send(format!("Produto: {id}")))
})?;
```

### Query String

```rust
// Example URL: /produtos?pagina=2
app.get("/produtos", |req: Request, res: Response| {
    let pagina = req.query("pagina").unwrap_or("1");
    Ok(res.send(format!("Página: {pagina}")))
})?;
```

### Request API

The `Request` type provides methods for accessing HTTP request data:

- `method()`: returns the HTTP method used.
- `path()`: returns the URL path without the query string.
- `header(name)`: looks up an HTTP header.
- `param(name)`: gets the value of a dynamic route parameter.
- `query(name)`: gets the value of a query string parameter.
- `cookies()`: returns the list of cookies received.
- `cookie(name)`: looks for a cookie by name.
- `body()`: provides the request body as bytes.
- `text()`: parses the body as UTF-8 text.
- `json<T>()`: converts the JSON body into a Rust structure that implements `DeserializeOwned`.
- `form()`: decodes URL-encoded or multipart/form-data fields while preserving order and repeated names.
- `files()`: returns files received through multipart/form-data.
- `file(field_name)`: finds the first file associated with the field name.
- `session()`: provides the session associated with the request when enabled.
- `regenerate_session()`: regenerates the current session identifier.
- `context()`: creates a copy of the request metadata for error handling.



### Response API

The `Response` type provides methods for constructing HTTP responses:

- `Response::new()`: creates a response with HTTP status 200.
- `.status(code)`: sets the HTTP status code.
- `.status_code()`: gets the current status code.
- `.send(body)`: sends plain text.
- `.html(body)`: sends HTML content.
- `.json(body)`: sends JSON content as text.
- `Response::error(code, message)`: creates a plain-text error response.
- `Response::json_error(code, message)`: creates a JSON error response.
- `.header(name, value)`: adds or replaces an HTTP header.
- `.send_bytes(bytes)`: sends binary content.
- `.render_file(path)`: loads an HTML file without processing variables.
- `.render_template(path, context)`: loads and processes a template with context.
- `.render(page, layout, partials_dir, context)`: renders a page with a layout and partials using strict validation.
- `Response::error_page(...)`: creates a custom error page.
- `.cookie(cookie, config)`: adds a cookie to the response.
- `.remove_cookie(name, config)`: sends a header to remove a cookie.
- `Response::redirect(path)`: creates an HTTP 302 redirect.
- `Response::redirect_with_status(code, path)`: creates a redirect with status 301, 302, 303, 307, or 308.

**Note about JSON:** the `.json(...)` method receives content in text format. To convert Rust structures to JSON, you can use `serde_json::json!(...).to_string()`.

**Note about cookies:** the `cookie()` and `remove_cookie()` methods receive a `SessionConfig` and return a `Result`, allowing header creation errors to be detected.

## WebSocket Communication

RustPress supports real-time bidirectional communication through the **WebSocket** protocol. This feature enables persistent connections between clients and servers, allowing messages to be exchanged without making additional HTTP requests.

WebSocket routes are registered through the `App::ws()` method.

### Create a WebSocket Route

The following example implements an echo server: every text message received is returned to the client.

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

Save the code in `examples/websocket.rs` and run it with:

```bash
cargo run --example websocket
```

The server will be available at `127.0.0.1:3000`, awaiting WebSocket connections at `/ws`.

### WebSocket API

RustPress provides the following types and methods:

| Item | Description |
|---|---|
| `App::ws(path, handler)` | Registers a WebSocket route. |
| `WebSocket::send(message)` | Sends a message. |
| `WebSocket::recv()` | Receives the next message. |
| `WebSocket::close()` | Closes the connection gracefully. |
| `Message::Text(String)` | UTF-8 text message. |
| `Message::Binary(Vec<u8>)` | Binary message. |
| `Message::Ping(Vec<u8>)` | Ping control message. |
| `Message::Pong(Vec<u8>)` | Pong control message. |
| `Message::Close` | Close message. |
| `WebSocketError` | Error type for WebSocket operations. |

The `recv()` method returns `Result<Option<Message>, WebSocketError>`. When it receives a close message, it returns `Ok(Some(Message::Close))`. When the stream ends without another message, it returns `Ok(None)`.

### Supported Features

- HTTP request handshake and upgrade to WebSocket.
- Text and binary message communication.
- Ping and Pong control messages.
- Routes with dynamic parameters, such as `/ws/:id`.
- Execution of global middlewares before the connection is accepted.
- Graceful closure and disconnect detection.
- Session integration: allows session data to be used during a WebSocket connection and saves changes made by the handler when it finishes.

**Note:** global middlewares can reject the handshake before the WebSocket connection is established.

## Password Security

RustPress provides the `security` module for protecting passwords with the **Argon2id** algorithm.

The API allows secure hashes to be generated and passwords to be verified without storing the original values.

### Generate a Hash

```rust
use rustpress_web::security::Password;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let password = "MinhaSenhaSegura123";

    let hash = Password::hash(password)?;

    println!("Hash: {hash}");

    Ok(())
}
```

### Verify a Password

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

### Custom Configuration

The default configuration uses Argon2id with the following parameters:

- Memory: 19,456 KiB (approximately 19 MiB);
- Iterations: 2;
- Parallelism: 1;
- Hash length: 32 bytes.

You can provide a custom configuration through `PasswordConfig` and the `Password::hash_with_config()` method. Values lower than the defaults may reduce the hash's resistance to brute-force attacks. Adjust these parameters according to server security and available resources.

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
## File Uploads

RustPress allows files to be received through HTTP requests with `multipart/form-data` content. The feature must be explicitly enabled through `App::use_uploads()`.

### Configure Storage

```rust
use rustpress_web::{App, upload::UploadConfig};

let mut app = App::new();

app.use_uploads(
    UploadConfig::new("storage/uploads")
)?;
```

The configuration allows limits to be set to protect the application:

| Method | Description | Default |
|---|---|---:|
| `with_max_total_size(size)` | Maximum total request size | 25 MiB |
| `with_max_file_size(size)` | Maximum size of each file | 10 MiB |
| `with_max_files(count)` | Maximum number of files | 5 |
| `with_max_fields(count)` | Maximum number of text fields | 32 |
| `with_max_field_size(size)` | Maximum size of each text field | 64 KiB |

Sizes are specified in bytes. Custom limits are validated before being accepted.

### Receive a File

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

The corresponding HTML form can be:

```html
<form action="/upload" method="POST" enctype="multipart/form-data">
    <input type="file" name="avatar">
    <button type="submit">Enviar ficheiro</button>
</form>
```

### Inspect Files and Fields

`Request` provides:

- `file("avatar")`: finds a file by field name.
- `files()`: returns the list of received files.
- `form()`: returns the text fields from the form.

Each `UploadedFile` provides the field name, original name, stored name, path, content type, and size.

### Security and Limitations

- Files are stored with unique names generated by the framework; the name supplied by the client is not used as the storage path.
- The original name is provided only as metadata and should not be considered trustworthy.
- Size and quantity limits are enforced during processing.
- If processing fails or a middleware rejects the request, temporary or already saved files from that processing are removed.
- Current processing receives the multipart body in memory before parsing fields and saving files. Therefore, it is not yet a streaming upload implementation.
## Sessions and Authentication

RustPress provides HTTP sessions through `App::use_sessions()`. By default, data is stored in memory (`MemoryStore`) and identified by a cookie named `rustpress_session`.

### Enable Sessions

Enable session management before starting the server:

```rust
use rustpress_web::{App, session::SessionConfig};

let mut app = App::new();
app.use_sessions(SessionConfig::default())?;
```

You can customize the cookie and session lifetime:

```rust
use rustpress_web::session::{SameSite, SessionConfig};

let config = SessionConfig {
    cookie_name: "minha_app_session".to_string(),
    max_age: 60 * 60, // one hour, in seconds
    http_only: true,
    secure: true, // production with HTTPS
    same_site: SameSite::Lax,
    ..SessionConfig::default()
};

app.use_sessions(config)?;
```

For local development with HTTP, keep `secure: false`. In production, use HTTPS and enable `secure: true`. The default storage is in memory: sessions do not survive process restarts and are not shared between multiple application instances.

### Persistent Storage with FileStore

In addition to in-memory storage, RustPress provides `FileStore`, which saves session data in a JSON file. Sessions can therefore be recovered after restarting the application.

To use this storage, configure it through `App::use_session_store()`:

```rust
use rustpress_web::{
    App,
    session::{FileStore, SessionConfig},
};

let mut app = App::new();

let store = FileStore::new("storage/sessions.json")?;

app.use_session_store(SessionConfig::default(), store)?;
```

To run a complete example with login and session recovery after restarting the server:

```bash
cargo run --example file_store
```

The application will be available at `http://127.0.0.1:3001`.

RustPress automatically creates the directories it needs to persist data.

**Important Considerations:**

- Session data and revocations are persisted in the JSON file.
- Sessions with an expiration time continue to respect expiration even after the application restarts.
- `FileStore` uses synchronous file operations, which can block an asynchronous execution thread.
- Read, write, and revoke operations acquire an exclusive lock and reload persisted state before modifying it, preventing concurrent updates from being lost.
- All instances sharing the storage must use the same file path and keep the auxiliary `.lock` file intact during use.
- `FileStore` uses synchronous file operations, which can block an asynchronous execution thread.

Use `MemoryStore` for applications that do not need persistence and `FileStore` when you want to preserve sessions across restarts.

### Create and Save Session Data

When sessions are enabled, each request receives a session. Insert or update data through `Session::insert()`:

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

RustPress automatically saves the modified session at the end of the request and sends the corresponding cookie in the response. It is not necessary to call a `save()` method in the handler.

### Retrieve Data and the User ID

Data is retrieved with `Session::get::<T>()`. The application should explicitly store the authenticated user's identifier, for example under the `user_id` key:

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

**Important:** `user_id` is application data stored during login. It is not the same as the technical session identifier. The latter can be accessed with `session.id()`, but should be treated as a secret: do not expose it in URLs, responses, public logs, or client-side code.

### Login Example with a Session

The following example demonstrates the complete flow. The fixed credentials are only for illustrating the integration; in a real application, look up the user in the database and verify the password with `security::Password::verify()`.

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

    // Demonstration: replace this with a database query
    // and Password::verify(&password, &stored_hash).
    if username != "demo" || password != "demo123" {
        return Ok(Response::redirect("/login?erro=credenciais"));
    }

    // Regenerates the ID after authentication, reducing the risk of session fixation.
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

Corresponding HTML form:

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

### Protect a Route

Check for `user_id` before returning private content:

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

For larger applications, you can also use `AuthMiddleware` and `RoleMiddleware`. The authentication middleware checks the `user_id` key; the role middleware checks the `role` key.

### End a Session (Logout)

Call `Session::destroy()` to destroy the current session:

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

When it detects a destroyed session, RustPress removes it from storage, revokes the identifier, and sends a removal cookie to the browser. You do not need to call `Response::remove_cookie()` manually to end a session managed by the framework.

### Remove Data Without Ending the Session

Use `Session::remove()` to delete a single key, or `Session::clear()` to clear all data without destroying the identifier:

```rust
// Remove only the user ID:
session.remove("user_id");

// Remove all data while keeping the session active:
session.clear();
```

Use `destroy()` for logout. Use `clear()` only when you want to keep the session active but remove the current data.

### Regenerate the Session Identifier

After authenticating the user, regenerate the identifier to reduce the risk of session fixation:

```rust
let regenerated = req.regenerate_session()
    .map_err(HandlerError::from_error)?;

if !regenerated {
    return Err(HandlerError::new("Gestão de sessões não está activada."));
}
```

Regeneration creates a new UUID, preserves the existing data, and revokes the previous identifier. The new cookie is sent when the modified session is finalized.

## HTTP Redirects

RustPress provides static methods in `Response` for redirecting the client to another path or URL.

### Simple Redirect

```rust
app.get("/antiga-pagina", |_req: Request, _res: Response| {
    Ok(Response::redirect("/nova-pagina"))
})?;
```

`Response::redirect(path)` creates an HTTP **302 Found** response, suitable for a temporary redirect.

### Choose the HTTP Status Code

Use `Response::redirect_with_status(code, path)` when you need to set the status explicitly:

```rust
// After a POST, redirect the browser to a GET page.
Ok(Response::redirect_with_status(303, "/resultado"))

// Temporary redirect that preserves the HTTP method and body.
Ok(Response::redirect_with_status(307, "/novo-endpoint"))

// Permanent redirect.
Ok(Response::redirect_with_status(301, "/novo-endereco"))
```

Accepted codes:

| Code | Meaning | Behavior |
|---:|---|---|
| 301 | Moved Permanently | Permanent redirect. |
| 302 | Found | Temporary redirect; the default for `Response::redirect()`. |
| 303 | See Other | Tells the client to retrieve the destination with GET; useful after POST/PUT/DELETE (Post/Redirect/Get). |
| 307 | Temporary Redirect | Temporary redirect that preserves the HTTP method and request body. |
| 308 | Permanent Redirect | Permanent redirect that preserves the HTTP method and request body. |

The destination can be a local path such as `/login` or a valid absolute URI. Do not construct destinations from untrusted user data: validate it and prefer known local paths to prevent open redirects.

## Route Groups

RustPress allows related routes to be organized into groups through `App::group()`. A group applies a common prefix to route paths and allows middlewares to be associated with all routes in the group.

This feature is useful for structuring APIs, separating public and private areas, and organizing modules such as administration, users, and products.

### Create a Route Group

The `group()` method accepts two arguments:

- `prefix`: common prefix for the routes in the group.
- `configure`: closure that receives a `RouteGroup` and registers the routes.

Example:

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

The registered routes are available at the following paths:

| Method | Final Path |
|---|---|
| GET | `/utilizadores/` |
| GET | `/utilizadores/:id` |
| POST | `/utilizadores/` |

The prefix is automatically combined with each route path. You can write route paths with or without the leading slash; RustPress normalizes the join.

### Available HTTP Methods

A `RouteGroup` supports the GET, POST, PUT, PATCH, and DELETE methods:

```rust
app.group("/api/produtos", |products| {
    products.get("/", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"GET"}"#))
    })?;

    products.post("/", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"POST"}"#))
    })?;

    products.put("/:id", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"PUT"}"#))
    })?;

    products.patch("/:id", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"PATCH"}"#))
    })?;

    products.delete("/:id", |_req, _res| {
        Ok(Response::json(r#"{"metodo":"DELETE"}"#))
    })?;

    Ok(())
})?;
```

All route registration methods (GET, POST, PUT, PATCH, and DELETE) accept handlers that return `Result<Response, HandlerError>`. Thus, any handler can return a normal response with `Ok()` or propagate an error with `Err()`.

For example:

```rust
use rustpress_web::HandlerError;

app.group("/api", |api| {
    api.put("/recurso/:id", |_req, _res| {
        Err(HandlerError::with_status(422, "Dados inválidos"))
    })?;

    api.patch("/recurso/:id", |_req, _res| {
        Err(HandlerError::with_status(422, "Não foi possível actualizar"))
    })?;

    api.delete("/recurso/:id", |_req, _res| {
        Err(HandlerError::with_status(404, "Recurso não encontrado"))
    })?;

    Ok(())
})?;
```

### Apply Middleware to a Group

Use `use_middleware()` to associate a simple middleware with every route in the group. If it returns `Some(Response)`, route execution stops; if it returns `None`, the request continues:

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

The example uses a fixed token only to demonstrate middleware behavior. In a real application, use secure authentication, sessions, or the authentication middlewares provided by RustPress.

For more advanced control, use `use_middleware_advanced()`. This method allows the chain to continue, respond directly, or return an error through `MiddlewareResult`:

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

### Nested Groups

You can create groups inside other groups. Prefixes are combined and middlewares are inherited in hierarchical order:

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

The example listing route is available at `GET /admin/utilizadores/`, and the individual route is at `GET /admin/utilizadores/:id`.

The middleware execution order is:

1. Global middlewares registered in the application.
2. Middlewares in the outer group (`/admin`).
3. Middlewares in the inner group (`/admin/utilizadores`).
4. Route handler.

A group's middlewares apply to every route in that group, even if they are declared after some routes inside the closure.

### Group Error Handling and Validation

The `group()` method returns `Result<(), String>`; therefore, use `?` or handle the error explicitly. RustPress validates paths and rejects conflicts or duplicate routes.

Group configuration is atomic: if the closure returns an error or a conflict occurs while registering routes, none of that group's routes are added to the application. This prevents the application from being left partially configured.

```rust
let result = app.group("/admin", |admin| {
    admin.get("/painel", |_req, _res| {
        Ok(Response::html("<h1>Painel</h1>"))
    })?;

    // Simulates a failure during configuration.
    Err("Falha ao configurar o grupo".to_string())
});

if let Err(error) = result {
    eprintln!("Não foi possível registar o grupo: {error}");
}
```

**Note:** groups organize routes and middlewares during application configuration. They do not create separate servers or alter access permissions on their own. To protect routes, attach appropriate authentication or authorization middlewares.

## Middleware

### Simple Middleware

A simple middleware receives a reference to the request. It returns `None` to let processing continue or `Some(Response)` to stop the request and respond immediately.

```rust
app.use_middleware(|req: &Request| {
    if req.path() == "/admin" {
        Some(Response::new().status(403).send("Acesso proibido"))
    } else {
        None
    }
});
```

### Advanced Middleware

An advanced middleware returns one of the `MiddlewareResult` values:

- `MiddlewareResult::Next`: continues to the next middleware or route;
- `MiddlewareResult::Respond(response)`: stops the chain and sends the response;
- `MiddlewareResult::Error(error)`: forwards the error to central handling.

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

The complete middleware logging example is in `examples/middleware_logging.rs`.

## Error Handling

RustPress provides `HandlerError` so a handler can return a failure instead of a normal response.

### Create and Return an Error

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

- `HandlerError::new(message)`: creates an HTTP 500 error.
- `HandlerError::with_status(status, message)`: creates an error with a status between 400 and 599; a code outside that range is converted to 500.
- `HandlerError::from_error(error)`: converts an external Rust error into a `HandlerError`.
- `From` conversions are also available for `String`, `&str`, `std::io::Error`, and `Box<dyn Error + Send + Sync>`.

### Global Error Handler

Configure `set_error_handler` to customize the response when a handler or advanced middleware forwards an error:

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

`RequestContext` allows access to the method, path, route parameters, query string, and available headers. Avoid returning internal messages, local paths, credentials, or database details to the client. Log those details only in appropriate logs.

The router also provides responses for routes that are not found (404) and methods that are not allowed (405). The corresponding templates are in `views/errors/`; the 500 page is used as the internal error fallback. If an error template cannot be loaded, the framework preserves the HTTP status code and returns a plain-text message.

See `examples/error_handler.rs` for a runnable example.

## Logger and Diagnostic Messages

RustPress is a Rust library, not a JavaScript environment: there is no global `console.log()` function. For occasional development messages, you can use the standard `println!` and `eprintln!` macros. For framework-managed structured logging, use `Logger`.

Available levels:

- `Level::Debug`
- `Level::Info`
- `Level::Warn`
- `Level::Error`

The minimum level can be set in the application with `app.set_log_level(...)` or in an independent logger with `logger.set_level(...)`.

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

The logger includes a UTC timestamp and ANSI colors. DEBUG/INFO are sent to stdout and WARN/ERROR to stderr. The server's internal logger records startup, HTTP requests (method, path, status, and duration), and connection errors. Each instance has its own configuration; clones share that instance's configuration.

Run `cargo run --example logger` to see the levels in action.

## Templates and Views

The template engine is based on HTML files and `TemplateContext`. Paths are relative to the process's current working directory. For the error pages and view example to work as documented, run the application from the project root and maintain the structure below.

### Recommended Structure

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

**The role of each folder:**

- `src/`: internal implementation of the library.
- `examples/`: small applications that demonstrate the API.
- `tests/`: integration tests.
- `public/`: static files exposed by `App::static_files`.
- `views/`: HTML pages and templates.
- `views/layouts/`: common structures that wrap the content of each page.
- `views/partials/`: reusable HTML components included in templates.
- `views/errors/`: pages used in router error responses.

### Files, Layouts, and Partials

The `examples/basic.rs` example configures the public folder and loads the initial page:

```rust
app.static_files("/assets", "public");

app.get("/", |_req: Request, res: Response| {
    Ok(res.render_file("views/index.html"))
})?;
```

With this configuration, `public/css/style.css` is available at `/assets/css/style.css`. In the HTML, use:

```html
<link rel="stylesheet" href="/assets/css/style.css">
```

To render a page with a layout and partials, the layout must contain the `{{{conteudo}}}` content marker. Partials are referenced by name without an extension and resolved inside the specified directory:

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

Create the files:

- `views/partials/cabecalho.html`
- `views/partials/rodape.html`
- a page, for example `views/pages/inicio.html` (the `pages` folder is an organizational convention; the path is passed explicitly to the method).

Render the page by specifying the three paths:

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

The `Response::render` method uses strict rendering: undefined variables and syntax errors switch the response to HTTP 500. For pages without a layout, use `render_template`; to return an HTML file without variable substitution, use `render_file`.

### Template Engine Syntax

| Syntax | Meaning |
|---|---|
| `{{titulo}}` | Substitutes a context variable. |
| `{{#if autenticado}}...{{/if}} ` | Shows the block if the value is true. |
| `{{else}}` | Alternative inside an `if` block. |
| `{{#each produtos}}...{{/each}} ` | Repeats the block for each element in a list. |
| `{{> cabecalho}}` | Includes `views/partials/cabecalho.html` (or the specified partials directory). |

Inside a loop, each object's properties are available by name; for simple elements, use `{{this}}`. Interpolated values are HTML-escaped. Blocks can be nested up to the internal depth limit; strict validation rejects malformed blocks, nonexistent variables, and circular partial inclusions.

Context example:

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

> **Important:** `TemplateContext` and the rendering methods receive filesystem paths. They are not URL paths. The working directory and the existence of layouts/partials affect rendering.

## Static Files

Register a URL prefix and the corresponding local folder:

```rust
app.static_files("/assets", "public");
```

The middleware serves GET and HEAD requests, determines the MIME type by extension, and rejects invalid paths or attempts to escape the configured public directory. Do not place secrets, configuration files, backups, or private data inside the public folder.

## Available Examples

| Example | Purpose | Command |
|---|---|---|
| `basic` | HTML page and static files | `cargo run --example basic` |
| `error_handler` | Global error handler | `cargo run --example error_handler` |
| `logger` | Logger levels | `cargo run --example logger` |
| `middleware_logging` | Advanced middleware and JSON responses | `cargo run --example middleware_logging` |
| `file_store` | Persistent sessions in a JSON file | `cargo run --example file_store` |
| `websocket` | Real-time bidirectional communication | `cargo run --example websocket` |

## Request Lifecycle

1. The Tokio server accepts a TCP connection and Hyper processes the HTTP request.
2. The router identifies the method and searches for the route matching the path.
3. Dynamic parameters are extracted and made available in `Request::param`.
4. Registered middlewares are executed.
5. The handler builds a response or returns a `HandlerError`.
6. Forwarded errors are handled by the global handler when configured.
7. The RustPress response is converted to a Hyper response and sent to the client.

## API Documentation (rustdoc)

The public API is documented directly in the modules and types under `src/` through Rustdoc comments (`///` and `//!`). The documentation includes descriptions, runnable examples, and notes about known limitations.

Generate the HTML documentation locally:

```bash
cargo doc --no-deps --open
```

Run the examples included in the documentation comments:

```bash
cargo test --doc
```

Verify the documentation and examples before preparing a release:

```bash
cargo fmt --check
cargo check
cargo test
cargo test --doc
cargo package --list
cargo package
```

The `#![warn(missing_docs)]` attribute is enabled in the crate to flag public items without documentation. Before release, fix all warnings and confirm that rustdoc examples compile.

## Tests

Run the complete test suite:

```bash
cargo test
```

To also validate the examples:

```bash
cargo check --examples
```

## Known Limitations and Future Features

Despite the features already implemented, some areas remain open for future development:

- **Database integration:** built-in support for database access and management.

These are potential future improvements to RustPress Web and should be accompanied by dedicated APIs and tests.

## Additional Documentation

- [RustPress Architecture](docs/ARCHITECTURE.md)
- [Contributing Guide](docs/CONTRIBUTING.md)
- [MIT License](LICENSE)

## Contributing

Contributions, suggestions, and bug reports are welcome. Before opening a pull request:

```bash
cargo fmt --check
cargo test
cargo check --examples
```

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for the workflow.

## License

This project is licensed under the MIT license. See [LICENSE](LICENSE).

## Author and Repository

- **Author:** Fabrício Michael
- **Repository:** https://github.com/fabriciomichael/rustpress-web
