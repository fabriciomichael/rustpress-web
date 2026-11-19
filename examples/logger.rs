use rustpress_web::{App, Level, Logger};

fn main() {
    let mut app: App = App::new();

    // Configure the application's logging level.
    app.set_log_level(Level::Debug);

    // Create an independent logger.
    let logger: Logger = Logger::new();
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
