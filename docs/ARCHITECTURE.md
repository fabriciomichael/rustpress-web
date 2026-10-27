# Arquitetura do RustPress

Este documento descreve a arquitetura interna do projecto e o papel dos seus principais módulos.

## Visão geral

O RustPress foi desenhado como um framework HTTP simples, estruturado em torno de três blocos principais:

- definição da aplicação;
- gestão de rotas e middleware;
- servidor HTTP assíncrono.

## Estrutura

### `App`

A estrutura `App` representa a aplicação web. É o ponto de entrada público e expõe métodos como `get`, `post`, `put`, `patch`, `delete` e `use_middleware`.

A `App` contém uma instância de `Router`, que é responsável pela associação entre método HTTP, caminho e callback.

### `Request`

`Request` encapsula a requisição recebida pelo Hyper e oferece acesso a:

- `method()`
- `path()`
- `header()`
- `param()`
- `query()`

A informação de parâmetros da rota é armazenada em `Vec<(String, String)>`, permitindo extração com base em nomes de parâmetro.

### `Response`

`Response` abstrai a mensagem de resposta HTTP para a aplicação. Permite manipular:

- status HTTP;
- corpo textual;
- headers;
- conteúdo JSON.

Antes de ser enviada ao cliente, a resposta é convertida para o tipo `hyper::Response`.

### `Router`

O `Router` guarda todas as rotas num `HashMap` indexado por `(Method, String)`. Cada rota tem uma função handler que recebe `Request` e `Response` e devolve `Response`.

A correspondência de rotas faz-se por:

- comparação do método HTTP;
- divisão do caminho em segmentos;
- matching por igualdade literal ou por marcadores `:param`.

Quando um caminho coincidente é encontrado, os parâmetros dinâmicos são extratos e passados ao handler.

### Middleware

O middleware é armazenado numa lista e executado antes do handler final. Se uma função de middleware devolver `Some(Response)`, a resposta é interrompida e enviada imediatamente.

Esta abordagem permite:

- autenticação;
- validação de acesso;
- logging de pedidos;
- manipulação simples de erros antes de o pedido chegar à rota.

### `server`

O módulo `server` cria um `TcpListener`, aceita ligações e usa o serviço `Hyper` para processar cada pedido.

Cada conexão é tratada em uma task Tokio separada, permitindo que o servidor mantenha um comportamento assíncrono e concorrente.

## Ciclo de execução

1. A aplicação é criada com `App::new()`.
2. As rotas e middleware são registados.
3. `listen` inicia o `TcpListener`.
4. O servidor aceita uma nova conexão.
5. O `Router` resolve a rota pretendida.
6. O middleware executa-se, se existir.
7. O handler correspondente gera uma resposta.
8. A resposta é convertida para Hyper e enviada ao cliente.

## Pontos de extensão

O projecto está preparado para receber extensões em áreas como:

- agrupamento de rotas por módulos;
- gestão de erros centralizada;
- suportes de templates e views;
- validação de payloads JSON;
- middlewares em cadeia com controlo de execução.

## Considerações de design

O design actual prioriza simplicidade, clareza e aprendizagem. A implementação mantém uma API mínima, permitindo que o project seja fácil de estudar e evoluir sem uma camada adicional muito pesada.

Em termos de arquitetura, o projecto está intencionalmente próximo do modelo clássico de framework web minimalista: rotas + handler + resposta.
