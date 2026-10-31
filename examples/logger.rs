use rustpress::{App, Level, Logger};

fn main() {
    let mut app = App::new();

    // Configurar o nível de registo da aplicação.
    app.set_log_level(Level::Debug);

    // Criar um logger independente.
    let logger = Logger::new();
    logger.set_level(Level::Debug);

    println!("--- Configuração DEBUG ---");

    logger.debug("Diagnóstico activado");
    logger.info("Aplicação iniciada");
    logger.warn("Aviso de teste");
    logger.error("Erro de teste");

    println!("\n--- Configuração ERROR ---");

    logger.set_level(Level::Error);

    logger.debug("Não deverá aparecer");
    logger.info("Não deverá aparecer");
    logger.warn("Não deverá aparecer");
    logger.error("Apenas este erro deverá aparecer");
}
