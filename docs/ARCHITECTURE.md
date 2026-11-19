# RustPress Web Architecture

This document describes the internal architecture of RustPress Web, its main components, and the HTTP request processing flow.

## 1. Overview

RustPress Web is a minimalist asynchronous HTTP framework for Rust, built on Hyper 1 and Tokio.

The architecture is organized around a public API for application definition, request processing, response construction, middleware, sessions, templates, logging, and error handling.

The `router` and `server` modules are internal and are not part of the public API.

## 2. Module Organization

| Module | Responsibility |
|---|---|
| `app` | Defines the application and provides methods for configuration and route registration. |
| `request` | Encapsulates HTTP request data and provides methods for accessing headers, parameters, body, and sessions. |
| `response` | Allows building HTTP responses, setting status codes, headers, cookies, redirects, and content. |
| `router` | Internally manages routes, middleware, sessions, and error handling. |
| `middleware` | Implements authentication middleware, role-based authorization, and CSRF protection. |
| `handler_error` | Defines the error structure used by handlers and the error handling system. |
| `session` | Manages sessions, cookies, configuration, storage, and session lifecycle. |
| `template` | Provides the template engine, variables, conditionals, loops, layouts, and partials. |
| `logger` | Provides logging levels and event logging mechanisms. |
| `server` | Internally implements the asynchronous TCP server and Hyper integration. |

## 3. App Component

The `App` structure represents the web application and serves as the main entry point of the public API.

It is responsible for registering routes and configuring the resources provided by the framework.

Its methods include:

- `App::new()` — creates a new application.
- `get()`, `post()`, `put()`, `patch()`, and `delete()` — register HTTP routes with handlers that return `Result<Response, HandlerError>`, allowing normal responses or error propagation.
- `use_middleware()` — registers middleware.
- `use_middleware_advanced()` — registers middleware with advanced execution control.
- `use_sessions()` — enables session management.
- `set_error_handler()` — configures the global error handler.
- `set_log_level()` — configures the logging level.
- `static_files()` — configures static file serving.
- `listen()` — starts the server on a specific port or address.
- `serve_listener()` — allows the application to be served through an existing `TcpListener`.

The `App` structure internally uses the `Router` to manage request processing.

## 4. Route Management

The `Router` is an internal component responsible for mapping HTTP methods and paths to their respective handlers.

Routes are stored internally using a structure based on the HTTP method and path.

The framework supports dynamic parameters using the `:param` syntax.

Example:

```rust
app.get("/users/:id", |req, _res| {
    let id = req.param("id").unwrap_or("unknown");

    Response::new().send(format!("User: {id}"))
});
```