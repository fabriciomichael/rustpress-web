# RustPress

RustPress é um framework HTTP leve para Rust, inspirado na simplicidade e ergonomia do Express.js. O objetivo do projecto é fornecer uma API directa para criar servidores web com rotas, parâmetros dinâmicos, consultas, middleware e respostas em texto ou JSON, mantendo uma base mínima e performante.

## Visão geral

Este projecto foi pensado para quem procura:

- criar APIs REST de forma simples em Rust;
- manter a sintaxe próxima de frameworks web populares;
- explorar a pilha HTTP em Rust com Hyper e Tokio;
- aprender a estrutura de um router, middleware e tratamento de pedidos.

## Funcionalidades

- Rotas HTTP por método: GET, POST, PUT, PATCH e DELETE
- Suporte a parâmetros dinâmicos em rotas, por exemplo `/utilizadores/:id`
- Leitura de query strings, como `?pagina=2&limite=10`
- Middleware global para pré-processamento do pedido
- Respostas simples e em JSON
- Servidor assíncrono baseado em Tokio + Hyper

## Requisitos

- Rust 1.85+ (ou uma versão compatível com a edição 2024)
- Cargo
- Sistema operativo compatível com Tokio/Hyper

## Instalação

```bash
cargo build
```

Para utilizar o projecto numa aplicação externa:

```bash
cargo add rustpress
```

Documentação adicional:

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)

## Arranque rápido

```rust
use rustpress::{App, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    app.get("/", |_req: Request, res: Response| {
        res.send("Bem-vindo ao RustPress")
    });

    app.get("/utilizadores/:id", |req: Request, res: Response| {
        let id = req.param("id").unwrap_or("desconhecido");
        res.send(format!("Utilizador: {id}"))
    });

    app.post("/utilizadores", |_req: Request, res: Response| {
        res.status(201).send("Utilizador criado")
    });

    app.listen("127.0.0.1:3000").await?;
    Ok(())
}
```

## Exemplos de uso

### Rota simples

```rust
app.get("/", |_req: Request, res: Response| {
    res.send("Olá do RustPress!")
});
```

### Rota com query string

```rust
app.get("/produtos", |req: Request, res: Response| {
    let pagina = req.query("pagina").unwrap_or("1");
    res.send(format!("Página: {pagina}"))
});
```

### Rota com parâmetro dinâmico

```rust
app.get("/utilizadores/:id", |req: Request, res: Response| {
    let id = req.param("id").unwrap_or("desconhecido");
    res.send(format!("Utilizador: {id}"))
});
```

### Resposta JSON

```rust
app.get("/api/utilizador", |_req: Request, res: Response| {
    res.json(r#"{"id":15,"nome":"Fabrício","activo":true}"#)
});
```

### Middleware

```rust
app.use_middleware(|req: &Request| {
    if req.path() == "/admin" {
        Some(Response::new().status(403).send("Acesso proibido"))
    } else {
        None
    }
});
```

## Estrutura do projecto

```text
rustpress/
├── Cargo.toml
├── README.md
├── examples/
│   └── basic.rs
├── src/
│   ├── app.rs
│   ├── lib.rs
│   ├── request.rs
│   ├── response.rs
│   ├── router.rs
│   └── server.rs
└── target/
```

### Componentes principais

- `App`: ponto de entrada da aplicação e API pública para definir rotas e middleware.
- `Request`: encapsula a requisição HTTP e expõe acesso a headers, path, query string e parâmetros de rota.
- `Response`: encapsula a resposta HTTP e inclui métodos para texto, JSON e manipulação de headers.
- `Router`: gere a correspondência de rotas, parâmetros dinâmicos e execução de middleware.
- `server`: inicia o servidor HTTP e aceita ligações em loop.

## Lifecycle de um pedido

1. O cliente envia uma requisição HTTP para o servidor.
2. O `Server` aceita a conexão e invoca o `Router`.
3. O `Router` compara o método e o caminho com as rotas registadas.
4. Se houver correspondência, extraem-se os parâmetros da rota.
5. Os middlewares são executados antes da rota principal.
6. A resposta é convertida para a estrutura HTTP do Hyper e enviada ao cliente.

## Roadmap

O projecto encontra-se em fase inicial e está preparado para evoluir com:

- suporte a rotas agrupadas por prefixos;
- gestão mais robusta de parâmetros e middlewares em cadeia;
- suporte a uploads e corpo de requisição estruturado;
- suporte a templates e views;
- testes automatizados de integração;
- documentação de API mais detalhada e exemplos adicionais.

## Contribuição

Contribuições são bem-vindas. Para colaborar:

```bash
git clone https://github.com/fabriciomichael/rustpress-web.git
cd rustpress
cargo test
```

Se quiser propor melhorias, novas rotas, correções ou exemplos, abra um pull request com uma descrição clara e focada.

## Licença

Este projecto está licenciado sob a licença MIT. Consulte o ficheiro [LICENSE](LICENSE) para mais detalhes.

## Contacto

- Autor: Fabrício Michael
- Repositório: https://github.com/fabriciomichael/rustpress
- Email: michaelfabricio59@gmail.com
