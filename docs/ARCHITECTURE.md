# Arquitectura do RustPress Web

Este documento descreve a arquitectura interna do RustPress Web, os seus principais componentes e o fluxo de processamento dos pedidos HTTP.

## 1. Visão geral

O RustPress Web é um framework HTTP assíncrono e minimalista para Rust, construído sobre Hyper 1 e Tokio.

A arquitectura organiza-se em torno de uma API pública para definição da aplicação, processamento de pedidos, construção de respostas, middleware, sessões, templates, logging e tratamento de erros.

Os módulos `router` e `server` são internos e não fazem parte da API pública.

## 2. Organização dos módulos

| Módulo | Responsabilidade |
|---|---|
| `app` | Define a aplicação e disponibiliza os métodos de configuração e registo de rotas. |
| `request` | Encapsula os dados do pedido HTTP e disponibiliza métodos para consultar cabeçalhos, parâmetros, corpo e sessões. |
| `response` | Permite construir respostas HTTP, definir status, cabeçalhos, cookies, redireccionamentos e conteúdo. |
| `router` | Gere internamente as rotas, os middlewares, as sessões e o tratamento de erros. |
| `middleware` | Implementa middlewares de autenticação, autorização por funções e protecção CSRF. |
| `handler_error` | Define a estrutura de erros utilizados pelos handlers e pelo sistema de tratamento de erros. |
| `session` | Gere sessões, cookies, configuração, armazenamento e ciclo de vida das sessões. |
| `template` | Disponibiliza o motor de templates, variáveis, condições, ciclos, layouts e partials. |
| `logger` | Disponibiliza os níveis e mecanismos de registo de eventos. |
| `server` | Implementa internamente o servidor TCP assíncrono e a integração com Hyper. |

## 3. Componente App

A estrutura `App` representa a aplicação web e constitui o principal ponto de entrada da API pública.

É responsável por registar rotas e configurar os recursos disponibilizados pelo framework.

Entre os seus métodos encontram-se:

- `App::new()` — cria uma nova aplicação.
- `get()` e `post()` — registam rotas HTTP.
- `put()`, `patch()` e `delete()` — registam rotas para os respectivos métodos HTTP.
- `put_fallible()`, `patch_fallible()` e `delete_fallible()` — registam handlers que podem devolver erros.
- `use_middleware()` — regista middleware.
- `use_middleware_advanced()` — regista middleware com controlo avançado de execução.
- `use_sessions()` — activa a gestão de sessões.
- `set_error_handler()` — configura o handler global de erros.
- `set_log_level()` — configura o nível de logging.
- `static_files()` — configura a disponibilização de ficheiros estáticos.
- `listen()` — inicia o servidor numa determinada porta ou endereço.
- `serve_listener()` — permite servir a aplicação através de um `TcpListener` existente.

A estrutura `App` utiliza internamente o `Router` para gerir o processamento dos pedidos.

## 4. Gestão de rotas

O `Router` é um componente interno responsável por associar os métodos e caminhos HTTP aos respectivos handlers.

As rotas são armazenadas internamente através de uma estrutura baseada no método HTTP e no caminho.

O framework suporta parâmetros dinâmicos, utilizando a sintaxe `:param`.

Exemplo:

```rust
app.get("/users/:id", |req, _res| {
    let id = req.param("id").unwrap_or("unknown");

    Response::new().send(format!("User: {id}"))
});