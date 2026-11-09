use rustpress_web::{App, HandlerError, MiddlewareResult, Request, Response, upload::UploadConfig};
use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;

fn request_with_multipart(app: App, boundary: &str, body: &[u8]) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Não foi possível reservar uma porta");

    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();

    let _server_thread = thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        runtime.block_on(async move {
            let listener = TcpListener::from_std(listener).unwrap();
            app.serve_listener(listener).await.unwrap();
        });
    });

    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if Instant::now() >= deadline {
            panic!("O servidor não ficou disponível a tempo");
        }

        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) {
            let headers = format!(
                "POST /upload HTTP/1.1\r\n\
                 Host: {address}\r\n\
                 Content-Type: multipart/form-data; boundary={boundary}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );

            stream.write_all(headers.as_bytes()).unwrap();
            stream.write_all(body).unwrap();

            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();

            return response;
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn temporary_directory() -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!(
        "rustpress-upload-test-{}-{timestamp}",
        std::process::id()
    ))
}

#[test]
fn receives_and_stores_file_through_http() {
    let directory = temporary_directory();
    std::fs::create_dir_all(&directory).unwrap();

    let mut app = App::new();

    app.use_uploads(UploadConfig::new(directory.clone()))
        .expect("Não foi possível activar os uploads");

    app.post("/upload", |request: Request, response: Response| {
        let file = request
            .file("avatar")
            .expect("O ficheiro avatar não foi recebido");

        let original_name = file.original_name().unwrap_or("");
        let size = file.size();
        let contents = std::fs::read(file.path()).unwrap();
        let contents = String::from_utf8_lossy(&contents);

        Ok(response.send(format!("{original_name}|{size}|{contents}")))
    })
    .expect("Não foi possível registar a rota");

    let boundary = "rustpress-test-boundary";

    let mut body = Vec::new();

    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"avatar\"; filename=\"foto.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n"
        )
        .as_bytes(),
    );

    body.extend_from_slice(b"avatar-data");

    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = request_with_multipart(app, boundary, &body);

    assert!(
        response.contains("200 OK"),
        "Resposta HTTP inesperada:\n{response}"
    );

    assert!(
        response.contains("foto.txt|11|avatar-data"),
        "O handler não recebeu os dados esperados:\n{response}"
    );

    let stored_files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert_eq!(
        stored_files.len(),
        1,
        "Deveria existir exactamente um ficheiro armazenado"
    );

    assert_eq!(std::fs::read(&stored_files[0]).unwrap(), b"avatar-data");

    std::fs::remove_dir_all(&directory).unwrap();
}
#[test]
fn receives_text_fields_and_files_in_same_multipart_request() {
    let directory = temporary_directory();
    std::fs::create_dir_all(&directory).unwrap();

    let mut app = App::new();

    app.use_uploads(UploadConfig::new(directory.clone()))
        .expect("Não foi possível activar os uploads");

    app.post("/upload", |request: Request, response: Response| {
        let description = request
            .form()
            .into_iter()
            .find(|(name, _)| name == "descricao")
            .map(|(_, value)| value)
            .unwrap_or_default();

        let file = request
            .file("avatar")
            .expect("O ficheiro avatar não foi recebido");

        Ok(response.send(format!(
            "{}|{}|{}",
            description,
            file.original_name().unwrap_or(""),
            file.size()
        )))
    })
    .expect("Não foi possível registar a rota");

    let boundary = "rustpress-fields-boundary";

    let mut body = Vec::new();

    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"descricao\"\r\n\r\n\
             Fotografia de perfil\r\n"
        )
        .as_bytes(),
    );

    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"avatar\"; filename=\"perfil.png\"\r\n\
             Content-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );

    body.extend_from_slice(b"PNG-TEST-DATA");

    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = request_with_multipart(app, boundary, &body);

    assert!(
        response.contains("200 OK"),
        "Resposta HTTP inesperada:\n{response}"
    );

    assert!(
        response.contains("Fotografia de perfil|perfil.png|13"),
        "Os campos ou o ficheiro não foram recebidos correctamente:\n{response}"
    );

    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn rejects_multipart_when_uploads_are_disabled() {
    let directory = temporary_directory();

    let mut app = App::new();

    app.post("/upload", |_request: Request, response: Response| {
        Ok(response.send("HANDLER_EXECUTED"))
    })
    .expect("Não foi possível registar a rota");

    let boundary = "rustpress-disabled-boundary";

    let mut body = Vec::new();

    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"avatar\"; filename=\"foto.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n"
        )
        .as_bytes(),
    );

    body.extend_from_slice(b"conteudo");

    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = request_with_multipart(app, boundary, &body);

    assert!(
        response.contains("400"),
        "Deveria rejeitar o pedido com HTTP 400:\n{response}"
    );

    assert!(
        !response.contains("HANDLER_EXECUTED"),
        "O handler não deveria ser executado:\n{response}"
    );

    assert!(
        !directory.exists(),
        "Nenhum directório de armazenamento deveria ser criado"
    );
}

#[test]
fn rejects_file_exceeding_configured_limit_through_http() {
    let directory = temporary_directory();
    std::fs::create_dir_all(&directory).unwrap();

    let mut app = App::new();

    let config = UploadConfig::new(directory.clone())
        .with_max_total_size(1024)
        .with_max_file_size(5)
        .with_max_field_size(512);

    app.use_uploads(config)
        .expect("Não foi possível activar os uploads");

    app.post("/upload", |_request: Request, response: Response| {
        Ok(response.send("HANDLER_EXECUTED"))
    })
    .expect("Não foi possível registar a rota");

    let boundary = "rustpress-size-boundary";

    let mut body = Vec::new();

    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"avatar\"; filename=\"grande.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n"
        )
        .as_bytes(),
    );

    body.extend_from_slice(b"123456");

    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = request_with_multipart(app, boundary, &body);

    assert!(
        response.contains("413"),
        "Deveria rejeitar o ficheiro com HTTP 413:\n{response}"
    );

    assert!(
        !response.contains("HANDLER_EXECUTED"),
        "O handler não deveria ser executado:\n{response}"
    );

    let remaining_files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert!(
        remaining_files.is_empty(),
        "Não deveriam permanecer ficheiros após a rejeição: {remaining_files:?}"
    );

    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn rejects_requests_with_too_many_files_and_cleans_storage() {
    let directory = temporary_directory();
    std::fs::create_dir_all(&directory).unwrap();

    let mut app = App::new();

    let config = UploadConfig::new(directory.clone()).with_max_files(1);

    app.use_uploads(config)
        .expect("Não foi possível activar os uploads");

    app.post("/upload", |_request: Request, response: Response| {
        Ok(response.send("HANDLER_EXECUTED"))
    })
    .expect("Não foi possível registar a rota");

    let boundary = "rustpress-max-files-boundary";

    let mut body = Vec::new();

    for (name, filename, contents) in [
        ("primeiro", "primeiro.txt", b"ficheiro-1".as_slice()),
        ("segundo", "segundo.txt", b"ficheiro-2".as_slice()),
    ] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\n\
                 Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\
                 Content-Type: text/plain\r\n\r\n"
            )
            .as_bytes(),
        );

        body.extend_from_slice(contents);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let response = request_with_multipart(app, boundary, &body);

    assert!(
        response.contains("400"),
        "Deveria rejeitar a requisição com HTTP 400:\n{response}"
    );

    assert!(
        !response.contains("HANDLER_EXECUTED"),
        "O handler não deveria ser executado:\n{response}"
    );

    let remaining_files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert!(
        remaining_files.is_empty(),
        "O primeiro ficheiro deveria ser removido após a rejeição: {remaining_files:?}"
    );

    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn rejects_requests_with_too_many_text_fields() {
    let directory = temporary_directory();
    std::fs::create_dir_all(&directory).unwrap();

    let mut app = App::new();

    let config = UploadConfig::new(directory.clone()).with_max_fields(1);

    app.use_uploads(config)
        .expect("Não foi possível activar os uploads");

    app.post("/upload", |_request: Request, response: Response| {
        Ok(response.send("HANDLER_EXECUTED"))
    })
    .expect("Não foi possível registar a rota");

    let boundary = "rustpress-max-fields-boundary";

    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"nome\"\r\n\r\n\
         Fabrício\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"descricao\"\r\n\r\n\
         Teste de limite\r\n\
         --{boundary}--\r\n"
    );

    let response = request_with_multipart(app, boundary, body.as_bytes());

    assert!(
        response.contains("400"),
        "Deveria rejeitar a requisição com HTTP 400:\n{response}"
    );

    assert!(
        !response.contains("HANDLER_EXECUTED"),
        "O handler não deveria ser executado:\n{response}"
    );

    let remaining_files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert!(
        remaining_files.is_empty(),
        "Não deveriam existir ficheiros armazenados: {remaining_files:?}"
    );

    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn removes_uploaded_files_when_middleware_rejects_request() {
    let dir = temporary_directory();
    let boundary = "middleware-cleanup-boundary";

    let mut app = App::new();

    app.use_uploads(UploadConfig::new(&dir))
        .expect("A configuração de uploads deveria ser válida");

    app.use_middleware(|_request| Some(Response::new().status(403).send("REJECTED")));

    app.post("/upload", |_request, _response| {
        Ok(Response::new().send("HANDLER_EXECUTED"))
    })
    .unwrap();

    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"avatar\"; filename=\"foto.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         avatar-data\r\n\
         --{boundary}--\r\n"
    );

    let response = request_with_multipart(app, boundary, body.as_bytes());

    assert!(
        response.starts_with("HTTP/1.1 403"),
        "Resposta inesperada: {response}"
    );

    assert!(
        !response.contains("HANDLER_EXECUTED"),
        "O handler não deveria ser executado"
    );

    assert!(dir.exists(), "A pasta de armazenamento deveria existir");

    let stored_files = std::fs::read_dir(&dir).unwrap().count();

    assert_eq!(
        stored_files, 0,
        "Os ficheiros deveriam ser removidos após a rejeição"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn removes_uploaded_files_when_middleware_returns_error() {
    let dir = temporary_directory();
    let boundary = "middleware-error-cleanup-boundary";

    let mut app = App::new();

    app.use_uploads(UploadConfig::new(&dir))
        .expect("A configuração de uploads deveria ser válida");

    app.use_middleware_advanced(|_request| {
        MiddlewareResult::Error(HandlerError::with_status(
            422,
            "Requisição rejeitada pelo middleware.",
        ))
    });

    app.post("/upload", |_request, _response| {
        Ok(Response::new().send("HANDLER_EXECUTED"))
    })
    .unwrap();

    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"avatar\"; filename=\"foto.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         avatar-data\r\n\
         --{boundary}--\r\n"
    );

    let response = request_with_multipart(app, boundary, body.as_bytes());

    assert!(
        response.starts_with("HTTP/1.1 422"),
        "Resposta inesperada: {response}"
    );

    assert!(
        !response.contains("HANDLER_EXECUTED"),
        "O handler não deveria ser executado"
    );

    let stored_files = std::fs::read_dir(&dir).unwrap().count();

    assert_eq!(
        stored_files, 0,
        "Os ficheiros deveriam ser removidos após o erro"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}
