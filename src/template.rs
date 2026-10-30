//! Motor de templates HTML do RustPress.
//!
//! [`TemplateContext`] armazena valores JSON e aplica-os a templates com
//! variáveis, condições, ciclos, layouts e partials. A interpolação escapa
//! valores para HTML; a renderização estrita devolve [`TemplateError`] quando
//! encontra variáveis indefinidas ou sintaxe inválida.
//!
//! # Exemplo
//!
//! ```
//! use rustpress::TemplateContext;
//! use serde_json::json;
//! let mut context = TemplateContext::new();
//! context.set("titulo", json!("Catálogo"));
//! context.set("mostrar", json!(true));
//! context.set("produtos", json!([{ "nome": "Teclado" }, { "nome": "Rato" }]));
//! let html = context.render("<h1>{{titulo}}</h1>{{#if mostrar}}{{#each produtos}}<p>{{nome}}</p>{{/each}}{{/if}}");
//! assert!(html.contains("Teclado"));
//! ```
//!
//! Os caminhos de páginas, layouts e partials são caminhos do sistema de
//! ficheiros, relativos ao directório de trabalho actual.

use serde_json::{Map, Value};
use std::fmt;

const MAX_TEMPLATE_DEPTH: usize = 20;

#[derive(Debug)]
pub enum TemplateError {
    UndefinedVariable(String),
    Syntax(String),
    NestingLimitExceeded,
    Io(std::io::Error),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable(name) => {
                write!(f, "Variable not defined in the template: {name}")
            }
            Self::Io(error) => {
                write!(f, "I/O error in template: {error}")
            }
            Self::Syntax(message) => {
                write!(f, "Syntax error in template: {message}")
            }
            Self::NestingLimitExceeded => {
                write!(f, "Maximum template nesting depth exceeded.")
            }
        }
    }
}

impl std::error::Error for TemplateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UndefinedVariable(_) => None,
            Self::Syntax(_) => None,
            Self::Io(error) => Some(error),
            Self::NestingLimitExceeded => None,
        }
    }
}

impl From<std::io::Error> for TemplateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Contexto de dados disponibilizado ao template.\n/// Armazena valores JSON para interpolação, condições e ciclos.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    data: Map<String, Value>,
}

impl TemplateContext {
    /// Cria um contexto vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adiciona ou substitui um valor.
    pub fn set(&mut self, name: impl Into<String>, value: Value) {
        self.data.insert(name.into(), value);
    }

    /// Obtém um valor pelo nome.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.data.get(name)
    }

    /// Devolve todos os dados do contexto.
    pub fn data(&self) -> &Map<String, Value> {
        &self.data
    }

    /// Valida a estrutura dos blocos de um template sem o renderizar.
    pub fn validate_template(template: &str) -> Result<(), TemplateError> {
        validate_template_syntax(template)
    }

    /// Renderiza variáveis e condições de um template.
    pub fn render(&self, template: &str) -> String {
        let mut protected_values = Vec::new();
        let prefix = "\u{E000}RUSTPRESS_NORMAL_PROTECTED_";

        // Evita conflitos com marcadores que já existam no template.
        let mut unique_prefix = prefix.to_string();
        while template.contains(&unique_prefix) {
            unique_prefix.push('_');
        }

        let rendered =
            self.render_with_protection(template, 0, &unique_prefix, &mut protected_values);

        restore_protected_values(&rendered, &unique_prefix, &protected_values)
    }

    fn render_with_protection(
        &self,
        template: &str,
        depth: usize,
        protected_prefix: &str,
        protected_values: &mut Vec<String>,
    ) -> String {
        if depth > MAX_TEMPLATE_DEPTH {
            return template.to_string();
        }

        let result =
            self.render_each_with_protection(template, depth, protected_prefix, protected_values);

        let result = self.render_conditions_with_protection(&result, depth);

        self.render_variables_with_protection(&result, protected_prefix, protected_values)
    }
    /// Renderiza um template e devolve erro se uma variável não existir.
    pub fn render_strict(&self, template: &str) -> Result<String, TemplateError> {
        let mut prefix = "\u{E000}RUSTPRESS_PROTECTED_".to_string();

        while template.contains(&prefix) {
            prefix.push('_');
        }

        let mut protected_values = Vec::new();

        let rendered =
            self.render_strict_with_depth(template, 0, &prefix, &mut protected_values)?;

        Ok(restore_protected_values(
            &rendered,
            &prefix,
            &protected_values,
        ))
    }

    fn render_strict_with_depth(
        &self,
        template: &str,
        depth: usize,
        protected_prefix: &str,
        protected_values: &mut Vec<String>,
    ) -> Result<String, TemplateError> {
        if depth > MAX_TEMPLATE_DEPTH {
            return Err(TemplateError::NestingLimitExceeded);
        }

        Self::validate_template(template)?;

        let result =
            self.render_each_strict(template, depth, protected_prefix, protected_values)?;
        let result = self.render_conditions_strict(&result, depth)?;

        let mut output = String::new();
        let mut position = 0;

        while let Some(relative_start) = result[position..].find("{{") {
            let start = position + relative_start;
            output.push_str(&result[position..start]);

            let value_start = start + 2;

            let Some(relative_end) = result[value_start..].find("}}") else {
                output.push_str(&result[start..]);
                return Ok(output);
            };

            let end = value_start + relative_end;
            let key = result[value_start..end].trim();

            if !Self::is_valid_variable_name(key) {
                output.push_str(&result[start..end + 2]);
                position = end + 2;
                continue;
            }

            match self.get(key) {
                Some(value) => {
                    let index = protected_values.len();

                    protected_values.push(escape_html(&value_to_string(value)));

                    output.push_str(&format!("{protected_prefix}{index}\u{E001}"));
                }
                None => {
                    return Err(TemplateError::UndefinedVariable(key.to_string()));
                }
            }

            position = end + 2;
        }

        output.push_str(&result[position..]);
        Ok(output)
    }

    fn render_conditions_with_protection(&self, template: &str, depth: usize) -> String {
        if depth > MAX_TEMPLATE_DEPTH {
            return template.to_string();
        }

        let mut output = String::new();
        let mut position = 0;

        while let Some(relative_start) = template[position..].find("{{#if ") {
            let start = position + relative_start;
            output.push_str(&template[position..start]);

            let opening_end = match template[start..].find("}}") {
                Some(end) => start + end,
                None => {
                    output.push_str(&template[start..]);
                    return output;
                }
            };

            let key = template[start + 6..opening_end].trim();
            let body_start = opening_end + 2;

            let (close_start, else_start) = match find_matching_if(template, body_start) {
                Some(found) => found,
                None => {
                    output.push_str(&template[start..]);
                    return output;
                }
            };

            let close_end = close_start + "{{/if}}".len();

            let selected = if self.is_truthy(key) {
                let end = else_start.unwrap_or(close_start);
                &template[body_start..end]
            } else if let Some(else_pos) = else_start {
                &template[else_pos + "{{else}}".len()..close_start]
            } else {
                ""
            };

            output.push_str(&self.render_conditions_with_protection(selected, depth + 1));

            position = close_end;
        }

        output.push_str(&template[position..]);
        output
    }

    /// Interpreta condições e rejeita variáveis inexistentes.
    fn render_conditions_strict(
        &self,
        template: &str,
        depth: usize,
    ) -> Result<String, TemplateError> {
        if depth > MAX_TEMPLATE_DEPTH {
            return Err(TemplateError::NestingLimitExceeded);
        }

        let mut output = String::new();
        let mut position = 0;

        while let Some(relative_start) = template[position..].find("{{#if ") {
            let start = position + relative_start;

            output.push_str(&template[position..start]);

            let opening_end = match template[start..].find("}}") {
                Some(end) => start + end,
                None => {
                    output.push_str(&template[start..]);
                    return Ok(output);
                }
            };

            let key = template[start + 6..opening_end].trim();

            if Self::is_valid_variable_name(key) && self.get(key).is_none() {
                return Err(TemplateError::UndefinedVariable(key.to_string()));
            }

            let body_start = opening_end + 2;

            let (close_start, else_start) = match find_matching_if(template, body_start) {
                Some(found) => found,
                None => {
                    output.push_str(&template[start..]);
                    return Ok(output);
                }
            };

            let close_end = close_start + "{{/if}}".len();

            let selected = if self.is_truthy(key) {
                let end = else_start.unwrap_or(close_start);
                &template[body_start..end]
            } else if let Some(else_pos) = else_start {
                &template[else_pos + "{{else}}".len()..close_start]
            } else {
                ""
            };

            output.push_str(&self.render_conditions_strict(selected, depth + 1)?);
            position = close_end;
        }

        output.push_str(&template[position..]);
        Ok(output)
    }

    fn render_variables_with_protection(
        &self,
        template: &str,
        protected_prefix: &str,
        protected_values: &mut Vec<String>,
    ) -> String {
        let mut output = String::new();
        let mut position = 0;

        while let Some(relative_start) = template[position..].find("{{") {
            let start = position + relative_start;
            output.push_str(&template[position..start]);

            let value_start = start + 2;

            let end = match template[value_start..].find("}}") {
                Some(end) => value_start + end,
                None => {
                    output.push_str(&template[start..]);
                    return output;
                }
            };

            let key = template[value_start..end].trim();

            if !Self::is_valid_variable_name(key) {
                output.push_str(&template[start..end + 2]);
                position = end + 2;
                continue;
            }

            if let Some(value) = self.get(key) {
                let index = protected_values.len();
                protected_values.push(escape_html(&value_to_string(value)));

                output.push_str(&format!("{protected_prefix}{index}\u{E001}"));
            } else {
                // Mantém expressões desconhecidas, tal como no modo normal.
                output.push_str(&template[start..end + 2]);
            }

            position = end + 2;
        }

        output.push_str(&template[position..]);
        output
    }

    /// Determina se um valor deve ser considerado verdadeiro.
    fn is_truthy(&self, key: &str) -> bool {
        match self.get(key) {
            Some(Value::Bool(value)) => *value,
            Some(Value::Null) | None => false,
            Some(Value::Number(value)) => value.as_f64().unwrap_or(0.0) != 0.0,
            Some(Value::String(value)) => !value.is_empty(),
            Some(Value::Array(value)) => !value.is_empty(),
            Some(Value::Object(value)) => !value.is_empty(),
        }
    }

    fn render_each_with_protection(
        &self,
        template: &str,
        depth: usize,
        protected_prefix: &str,
        protected_values: &mut Vec<String>,
    ) -> String {
        if depth > MAX_TEMPLATE_DEPTH {
            return template.to_string();
        }

        let mut output = String::new();
        let mut position = 0;

        while let Some(relative_start) = template[position..].find("{{#each ") {
            let start = position + relative_start;
            output.push_str(&template[position..start]);

            let opening_end = match template[start..].find("}}") {
                Some(end) => start + end,
                None => {
                    output.push_str(&template[start..]);
                    return output;
                }
            };

            let key = template[start + 8..opening_end].trim();
            let body_start = opening_end + 2;

            let (close_start, close_end) = match find_matching_each(template, body_start) {
                Some(found) => found,
                None => {
                    output.push_str(&template[start..]);
                    return output;
                }
            };

            let body = &template[body_start..close_start];

            if let Some(Value::Array(items)) = self.get(key) {
                for item in items {
                    let mut child = self.clone();

                    if let Value::Object(properties) = item {
                        for (name, value) in properties {
                            child.set(name.clone(), value.clone());
                        }
                    } else {
                        child.set("this", item.clone());
                    }

                    output.push_str(&child.render_with_protection(
                        body,
                        depth + 1,
                        protected_prefix,
                        protected_values,
                    ));
                }
            }

            position = close_end;
        }

        output.push_str(&template[position..]);
        output
    }

    fn render_each_strict(
        &self,
        template: &str,
        depth: usize,
        protected_prefix: &str,
        protected_values: &mut Vec<String>,
    ) -> Result<String, TemplateError> {
        let mut output = String::new();
        let mut position = 0;

        while let Some(relative_start) = template[position..].find("{{#each ") {
            let start = position + relative_start;
            output.push_str(&template[position..start]);

            let opening_end = match template[start..].find("}}") {
                Some(end) => start + end,
                None => {
                    output.push_str(&template[start..]);
                    return Ok(output);
                }
            };

            let key = template[start + 8..opening_end].trim();
            let body_start = opening_end + 2;

            let (close_start, close_end) = match find_matching_each(template, body_start) {
                Some(found) => found,
                None => {
                    output.push_str(&template[start..]);
                    return Ok(output);
                }
            };

            let body = &template[body_start..close_start];

            match self.get(key) {
                Some(Value::Array(items)) => {
                    for item in items {
                        let mut child = self.clone();

                        if let Value::Object(properties) = item {
                            for (name, value) in properties {
                                child.set(name.clone(), value.clone());
                            }
                        } else {
                            child.set("this", item.clone());
                        }

                        output.push_str(&child.render_strict_with_depth(
                            body,
                            depth + 1,
                            protected_prefix,
                            protected_values,
                        )?);
                    }
                }
                Some(_) => {
                    // O valor existe, mas não é uma coleção.
                }
                None => {
                    return Err(TemplateError::UndefinedVariable(key.to_string()));
                }
            }

            position = close_end;
        }

        output.push_str(&template[position..]);
        Ok(output)
    }

    /// Prepara uma página com layout e partials.
    fn prepare_layout(
        &self,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
    ) -> std::io::Result<String> {
        let page = std::fs::read_to_string(page_path)?;
        let layout = std::fs::read_to_string(layout_path)?;

        let combined = layout.replace("{{{conteudo}}}", &page);

        expand_partials(&combined, partials_dir)
    }

    /// Compõe uma página usando um layout e partials reutilizáveis.
    pub fn render_with_layout(
        &self,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
    ) -> std::io::Result<String> {
        self.prepare_layout(page_path, layout_path, partials_dir)
            .map(|template| self.render(&template))
    }
    /// Verifica se o nome de uma variável é válido.
    fn is_valid_variable_name(key: &str) -> bool {
        let mut chars = key.chars();

        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
            _ => return false,
        }

        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// Compõe uma página com layout e partials em modo estrito.
    pub fn render_with_layout_strict(
        &self,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
    ) -> Result<String, TemplateError> {
        let template = self.prepare_layout(page_path, layout_path, partials_dir)?;

        self.render_strict(&template)
    }
}

fn validate_template_syntax(template: &str) -> Result<(), TemplateError> {
    // Cada bloco guarda o seu tipo e se já encontrou um else.
    let mut stack: Vec<(&str, bool)> = Vec::new();
    let mut position = 0;

    while let Some(relative_start) = template[position..].find("{{") {
        let start = position + relative_start;

        let Some(relative_end) = template[start + 2..].find("}}") else {
            return Err(TemplateError::Syntax(
                "Expression without a closing element '}}'.".to_string(),
            ));
        };

        let end = start + 2 + relative_end;
        let expression = template[start + 2..end].trim();

        if expression.is_empty() {
            return Err(TemplateError::Syntax(
                "Empty template expression.".to_string(),
            ));
        }

        if let Some(condition) = expression.strip_prefix("#if ") {
            let condition = condition.trim();

            if !TemplateContext::is_valid_variable_name(condition) {
                return Err(TemplateError::Syntax(
                    "Invalid variable name in if condition.".to_string(),
                ));
            }

            stack.push(("if", false));
            if stack.len() > MAX_TEMPLATE_DEPTH {
                return Err(TemplateError::NestingLimitExceeded);
            }
        } else if let Some(collection) = expression.strip_prefix("#each ") {
            let collection = collection.trim();

            if !TemplateContext::is_valid_variable_name(collection) {
                return Err(TemplateError::Syntax(
                    "Invalid collection name in each block.".to_string(),
                ));
            }

            stack.push(("each", false));
            if stack.len() > MAX_TEMPLATE_DEPTH {
                return Err(TemplateError::NestingLimitExceeded);
            }
        } else if expression == "#if" || expression == "#each" {
            return Err(TemplateError::Syntax(
                "Missing variable name in block expression.".to_string(),
            ));
        } else if expression == "else" {
            match stack.last_mut() {
                Some(("if", has_else)) => {
                    if *has_else {
                        return Err(TemplateError::Syntax(
                            "Duplicate 'else' in the same if block.".to_string(),
                        ));
                    }

                    *has_else = true;
                }
                Some((block, _)) => {
                    return Err(TemplateError::Syntax(format!(
                        "'else' is not allowed inside the '{block}' block."
                    )));
                }
                None => {
                    return Err(TemplateError::Syntax(
                        "'else' without an open if block.".to_string(),
                    ));
                }
            }
        } else if expression.starts_with("else ") || expression.starts_with("else\t") {
            return Err(TemplateError::Syntax(
                "Invalid 'else' expression. The 'else' directive must appear alone.".to_string(),
            ));
        } else if expression == "/if" {
            match stack.pop() {
                Some(("if", _)) => {}
                Some((other, _)) => {
                    return Err(TemplateError::Syntax(format!(
                        "'/if' closing tag incompatible with the block '{other}'."
                    )));
                }
                None => {
                    return Err(TemplateError::Syntax(
                        "'/if' closing tag with no open block.".to_string(),
                    ));
                }
            }
        } else if expression == "/each" {
            match stack.pop() {
                Some(("each", _)) => {}
                Some((other, _)) => {
                    return Err(TemplateError::Syntax(format!(
                        "'/each' closing tag is incompatible with the block '{other}'."
                    )));
                }
                None => {
                    return Err(TemplateError::Syntax(
                        "Closing '/each' without an open block.".to_string(),
                    ));
                }
            }
        } else if expression.starts_with('#') || expression.starts_with('/') {
            return Err(TemplateError::Syntax(format!(
                "Unsupported template block expression: '{expression}'."
            )));
        } else if !TemplateContext::is_valid_variable_name(expression) {
            return Err(TemplateError::Syntax(format!(
                "Invalid template variable name: '{expression}'."
            )));
        }

        position = end + 2;
    }

    if let Some((block, _)) = stack.last() {
        return Err(TemplateError::Syntax(format!(
            "Block '{block}' without a closing tag."
        )));
    }

    Ok(())
}

/// Encontra o fecho de um bloco if, respeitando blocos aninhados.
fn find_matching_if(template: &str, body_start: usize) -> Option<(usize, Option<usize>)> {
    let mut position = body_start;
    let mut depth = 0usize;
    let mut else_start = None;

    loop {
        let next_open = template[position..].find("{{#if ").map(|i| position + i);

        let next_else = template[position..].find("{{else}}").map(|i| position + i);

        let next_close = template[position..].find("{{/if}}").map(|i| position + i);

        let next = [next_open, next_else, next_close]
            .into_iter()
            .flatten()
            .min()?;

        if Some(next) == next_open {
            depth += 1;
            position = next + "{{#if ".len();
        } else if Some(next) == next_else {
            if depth == 0 && else_start.is_none() {
                else_start = Some(next);
            }

            position = next + "{{else}}".len();
        } else {
            if depth == 0 {
                return Some((next, else_start));
            }

            depth -= 1;
            position = next + "{{/if}}".len();
        }
    }
}

fn find_matching_each(template: &str, body_start: usize) -> Option<(usize, usize)> {
    let mut position = body_start;
    let mut depth = 0usize;

    loop {
        let next_open = template[position..].find("{{#each ").map(|i| position + i);

        let next_close = template[position..].find("{{/each}}").map(|i| position + i);

        let next = [next_open, next_close].into_iter().flatten().min()?;

        if Some(next) == next_open {
            depth += 1;
            position = next + "{{#each ".len();
        } else {
            if depth == 0 {
                let close_end = next + "{{/each}}".len();
                return Some((next, close_end));
            }

            depth -= 1;
            position = next + "{{/each}}".len();
        }
    }
}

/// Converte valores JSON para texto.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

/// Escapa caracteres especiais de HTML.
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Substitui {{> nome}} pelo conteúdo de views/partials/nome.html.
fn expand_partials(template: &str, partials_dir: &str) -> std::io::Result<String> {
    fn expand(
        template: &str,
        partials_dir: &std::path::Path,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> std::io::Result<String> {
        const MAX_DEPTH: usize = 20;

        if depth > MAX_DEPTH {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Profundidade máxima de inclusões de partials excedida",
            ));
        }

        let mut result = String::with_capacity(template.len());
        let mut position = 0;

        while let Some(relative_start) = template[position..].find("{{>") {
            let start = position + relative_start;
            result.push_str(&template[position..start]);

            let Some(relative_end) = template[start + 3..].find("}}") else {
                result.push_str(&template[start..]);
                return Ok(result);
            };

            let end = start + 3 + relative_end;
            let name = template[start + 3..end].trim();

            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Nome de partial inválido",
                ));
            }

            if stack.iter().any(|item| item == name) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Inclusão circular de partial detectada: {name}"),
                ));
            }

            let path = partials_dir.join(format!("{name}.html"));
            let canonical_path = std::fs::canonicalize(&path)?;

            if !canonical_path.starts_with(partials_dir) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "O partial tenta aceder a um ficheiro fora do diretório autorizado",
                ));
            }

            let partial = std::fs::read_to_string(canonical_path)?;

            stack.push(name.to_string());
            let expanded = expand(&partial, partials_dir, stack, depth + 1);
            stack.pop();

            result.push_str(&expanded?);
            position = end + 2;
        }

        result.push_str(&template[position..]);
        Ok(result)
    }

    let canonical_partials_dir = std::fs::canonicalize(partials_dir)?;

    expand(template, &canonical_partials_dir, &mut Vec::new(), 0)
}

fn restore_protected_values(template: &str, prefix: &str, values: &[String]) -> String {
    let mut output = String::new();
    let mut position = 0;

    while let Some(relative_start) = template[position..].find(prefix) {
        let start = position + relative_start;

        output.push_str(&template[position..start]);

        let index_start = start + prefix.len();

        let Some(relative_end) = template[index_start..].find('\u{E001}') else {
            output.push_str(&template[start..]);
            return output;
        };

        let index_end = index_start + relative_end;
        let marker_end = index_end + '\u{E001}'.len_utf8();

        let index = template[index_start..index_end].parse::<usize>();

        if let Ok(index) = index
            && let Some(value) = values.get(index)
        {
            output.push_str(value);
            position = marker_end;
            continue;
        }

        output.push_str(&template[start..marker_end]);
        position = marker_end;
    }

    output.push_str(&template[position..]);
    output
}

#[cfg(test)]
mod tests {

    use super::*;
    use serde_json::json;

    #[test]
    fn escapes_html_in_variables() {
        let mut context = TemplateContext::new();
        context.set("nome", json!("<script>alert('xss')</script>"));

        let result = context.render("Olá, {{nome}}");

        assert_eq!(
            result,
            "Olá, &lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn escapes_html_entities_without_double_escaping() {
        let mut context = TemplateContext::new();
        context.set("texto", json!("A & B < C"));
        context.set("entidades", json!("&lt;script&gt;"));

        let result = context.render("{{texto}} | {{entidades}}");

        assert_eq!(result, "A &amp; B &lt; C | &amp;lt;script&amp;gt;");
    }

    #[test]
    fn escapes_all_special_html_characters() {
        let mut context = TemplateContext::new();
        context.set("texto", json!("& < > \" '"));

        let result = context.render("{{texto}}");

        assert_eq!(result, "&amp; &lt; &gt; &quot; &#x27;");
    }

    #[test]
    fn normal_render_limits_nested_if_depth() {
        let mut context = TemplateContext::new();
        context.set("ativo", serde_json::json!(true));

        let mut template = String::new();

        for _ in 0..25 {
            template.push_str("{{#if ativo}}");
        }

        template.push_str("Conteúdo");

        for _ in 0..25 {
            template.push_str("{{/if}}");
        }

        let result = context.render(&template);

        assert!(result.contains("{{#if ativo}}"));
        assert!(result.contains("Conteúdo"));
        assert!(result.contains("{{/if}}"));
    }

    #[test]
    fn normal_render_does_not_reinterpret_values_inside_each() {
        let mut context = TemplateContext::new();

        context.set(
            "mensagens",
            vec!["{{segredo}}", "<script>alert(1)</script>"].into(),
        );
        context.set("segredo", "VALOR_PRIVADO".into());

        let result = context.render("{{#each mensagens}}{{this}}|{{/each}}");

        assert_eq!(result, "{{segredo}}|&lt;script&gt;alert(1)&lt;/script&gt;|");
    }

    #[test]
    fn renders_if_condition_when_true() {
        let mut context = TemplateContext::new();
        context.set("autenticado", json!(true));

        let result = context.render("{{#if autenticado}}Bem-vindo{{else}}Inicia sessão{{/if}}");

        assert_eq!(result, "Bem-vindo");
    }

    #[test]
    fn renders_else_when_condition_is_false() {
        let mut context = TemplateContext::new();
        context.set("autenticado", json!(false));

        let result = context.render("{{#if autenticado}}Bem-vindo{{else}}Inicia sessão{{/if}}");

        assert_eq!(result, "Inicia sessão");
    }

    #[test]
    fn renders_else_when_if_variable_is_missing() {
        let context = TemplateContext::new();

        let result =
            context.render("{{#if utilizador_inexistente}}Bem-vindo{{else}}Visitante{{/if}}");

        assert_eq!(result, "Visitante");
    }

    #[test]
    fn evaluates_truthiness_for_different_value_types() {
        let mut context = TemplateContext::new();

        context.set("nulo", json!(null));
        context.set("zero", json!(0));
        context.set("texto_zero", json!("0"));
        context.set("texto_vazio", json!(""));
        context.set("texto", json!("Olá"));
        context.set("lista_vazia", json!([]));
        context.set("lista", json!([1]));
        context.set("objeto_vazio", json!({}));
        context.set("objeto", json!({"nome": "Ana"}));

        assert_eq!(context.render("{{#if nulo}}SIM{{else}}NÃO{{/if}}"), "NÃO");
        assert_eq!(context.render("{{#if zero}}SIM{{else}}NÃO{{/if}}"), "NÃO");
        assert_eq!(
            context.render("{{#if texto_zero}}SIM{{else}}NÃO{{/if}}"),
            "SIM"
        );
        assert_eq!(
            context.render("{{#if texto_vazio}}SIM{{else}}NÃO{{/if}}"),
            "NÃO"
        );
        assert_eq!(context.render("{{#if texto}}SIM{{else}}NÃO{{/if}}"), "SIM");
        assert_eq!(
            context.render("{{#if lista_vazia}}SIM{{else}}NÃO{{/if}}"),
            "NÃO"
        );
        assert_eq!(context.render("{{#if lista}}SIM{{else}}NÃO{{/if}}"), "SIM");
        assert_eq!(
            context.render("{{#if objeto_vazio}}SIM{{else}}NÃO{{/if}}"),
            "NÃO"
        );
        assert_eq!(context.render("{{#if objeto}}SIM{{else}}NÃO{{/if}}"), "SIM");
    }

    #[test]
    fn renders_each_and_escapes_values() {
        let mut context = TemplateContext::new();
        context.set(
            "produtos",
            json!([
                {"nome": "Teclado"},
                {"nome": "<script>"}
            ]),
        );

        let result = context.render("{{#each produtos}}<p>{{nome}}</p>{{/each}}");

        assert_eq!(result, "<p>Teclado</p><p>&lt;script&gt;</p>");
    }

    #[test]
    fn rejects_invalid_partial_names() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("rustpress-invalid-partial-{unique}"));
        fs::create_dir_all(&dir).unwrap();

        let result = expand_partials("{{> ../secret}}", dir.to_str().unwrap());

        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn rejects_circular_partials() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("rustpress-partials-{unique}"));

        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("a.html"), "{{> b}}").unwrap();
        fs::write(dir.join("b.html"), "{{> a}}").unwrap();

        let result = expand_partials("{{> a}}", dir.to_str().unwrap());

        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn expands_nested_partials() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("rustpress-nested-{unique}"));
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("cabecalho.html"), "<header>Olá</header>").unwrap();
        fs::write(
            dir.join("pagina.html"),
            "{{> cabecalho}}<main>Conteúdo</main>",
        )
        .unwrap();

        let result = expand_partials("{{> pagina}}", dir.to_str().unwrap());

        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(result.unwrap(), "<header>Olá</header><main>Conteúdo</main>");
    }

    #[test]
    fn rejects_partials_exceeding_max_depth() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("rustpress-depth-{unique}"));
        fs::create_dir_all(&dir).unwrap();

        for index in 0..22 {
            let content = if index == 21 {
                "Fim".to_string()
            } else {
                format!("{{{{> partial_{}}}}}", index + 1)
            };

            fs::write(dir.join(format!("partial_{index}.html")), content).unwrap();
        }

        let result = expand_partials("{{> partial_0}}", dir.to_str().unwrap());

        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn partials_respect_exact_max_depth_boundary() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("rustpress-depth-boundary-{unique}"));

        fs::create_dir_all(&dir).unwrap();

        // Vinte inclusões devem ser permitidas.
        for index in 0..20 {
            let content = if index == 19 {
                "Fim".to_string()
            } else {
                format!("{{{{> partial_{}}}}}", index + 1)
            };

            fs::write(dir.join(format!("partial_{index}.html")), content).unwrap();
        }

        let result = expand_partials("{{> partial_0}}", dir.to_str().unwrap());
        assert_eq!(result.unwrap(), "Fim");

        // A vigésima primeira inclusão deve ser rejeitada.
        fs::write(dir.join("partial_19.html"), "{{> partial_20}}").unwrap();
        fs::write(dir.join("partial_20.html"), "Fim").unwrap();

        let result = expand_partials("{{> partial_0}}", dir.to_str().unwrap());

        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_partial_symlink_outside_directory() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-symlink-{unique}"));
        let partials = base.join("partials");
        let outside = base.join("outside.html");

        fs::create_dir_all(&partials).unwrap();
        fs::write(&outside, "Conteúdo externo").unwrap();

        let link = partials.join("externo.html");

        #[cfg(windows)]
        {
            if let Err(error) = std::os::windows::fs::symlink_file(&outside, &link) {
                fs::remove_dir_all(&base).unwrap();

                eprintln!("Teste ignorado: o Windows não permitiu criar o symlink: {error}");

                return;
            }
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let result = expand_partials("{{> externo}}", partials.to_str().unwrap());

        fs::remove_dir_all(&base).unwrap();

        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn strict_render_rejects_excessive_nested_if_depth() {
        let mut context = TemplateContext::new();

        context.set("ativo", serde_json::json!(true));

        let mut template = String::new();

        for _ in 0..25 {
            template.push_str("{{#if ativo}}");
        }

        template.push_str("Conteúdo");

        for _ in 0..25 {
            template.push_str("{{/if}}");
        }

        let result = context.render_strict(&template);

        assert!(matches!(result, Err(TemplateError::NestingLimitExceeded)));
    }

    #[test]
    fn strict_render_rejects_excessive_nested_each_depth() {
        let mut context = TemplateContext::new();

        let mut template = String::new();

        for index in 0..25 {
            let name = format!("items{index}");
            context.set(name, serde_json::json!([1]));

            template.push_str(&format!("{{{{#each items{index}}}}}"));
        }

        template.push_str("Conteúdo");

        for _ in 0..25 {
            template.push_str("{{/each}}");
        }

        let result = context.render_strict(&template);

        assert!(matches!(result, Err(TemplateError::NestingLimitExceeded)));
    }

    #[test]
    fn accepts_valid_variable_names() {
        assert!(TemplateContext::is_valid_variable_name("nome"));
        assert!(TemplateContext::is_valid_variable_name("_nome"));
        assert!(TemplateContext::is_valid_variable_name("usuario123"));
        assert!(TemplateContext::is_valid_variable_name("nome_completo"));
    }

    #[test]
    fn rejects_invalid_variable_names() {
        assert!(!TemplateContext::is_valid_variable_name(""));
        assert!(!TemplateContext::is_valid_variable_name("user.name"));
        assert!(!TemplateContext::is_valid_variable_name("../secret"));
        assert!(!TemplateContext::is_valid_variable_name("nome-completo"));
        assert!(!TemplateContext::is_valid_variable_name("123nome"));
    }

    #[test]
    fn variable_values_are_not_reinterpreted_as_template_expressions() {
        let mut context = TemplateContext::new();
        context.set("texto", json!("{{segredo}} <script>"));
        context.set("segredo", json!("CONTEÚDO PRIVADO"));

        let result = context.render("{{texto}}");

        assert_eq!(result, "{{segredo}} &lt;script&gt;");
    }

    #[test]
    fn preserves_unknown_variables_for_debugging() {
        let context = TemplateContext::new();
        let result = context.render("Olá, {{usuario}}!");

        assert_eq!(result, "Olá, {{usuario}}!");
    }

    #[test]
    fn strict_render_rejects_unknown_variables() {
        let context = TemplateContext::new();

        let result = context.render_strict("Olá, {{usuario}}!");

        assert!(matches!(
            result,
            Err(TemplateError::UndefinedVariable(name)) if name == "usuario"
        ));
    }

    #[test]
    fn strict_render_escapes_html() {
        let mut context = TemplateContext::new();
        context.set(
            "nome",
            Value::String("<script>alert(1)</script>".to_string()),
        );

        let result = context.render_strict("{{nome}}").unwrap();

        assert_eq!(result, "&lt;script&gt;alert(1)&lt;/script&gt;");
    }

    #[test]
    fn strict_render_rejects_nested_opening_delimiters() {
        let context = TemplateContext::new();

        let result = context.render_strict("{{nome {{idade}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn normal_render_preserves_unknown_variables() {
        let context = TemplateContext::new();

        assert_eq!(context.render("Olá, {{usuario}}!"), "Olá, {{usuario}}!");
    }

    #[test]
    fn strict_layout_render_rejects_unknown_variables() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-strict-layout-{unique}"));
        let partials = base.join("partials");

        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        fs::write(&page, "<h1>{{usuario}}</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let context = TemplateContext::new();

        let result = context.render_with_layout_strict(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
        );

        assert!(matches!(
            result,
            Err(TemplateError::UndefinedVariable(name)) if name == "usuario"
        ));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn strict_layout_render_returns_io_error_for_missing_page() {
        let context = TemplateContext::new();

        let result = context.render_with_layout_strict(
            "rustpress-page-that-does-not-exist-83721.html",
            "layout.html",
            "partials",
        );

        assert!(matches!(result, Err(TemplateError::Io(_))));
    }

    #[test]
    fn strict_render_rejects_unknown_variable_inside_each() {
        use serde_json::json;

        let mut context = TemplateContext::new();

        context.set(
            "produtos",
            json!([
                {"nome": "Teclado"}
            ]),
        );

        let template = "{{#each produtos}}<p>{{nome}} - {{preco}}</p>{{/each}}";

        let result = context.render_strict(template);

        assert!(matches!(
            result,
            Err(TemplateError::UndefinedVariable(name))
                if name == "preco"
        ));
    }

    #[test]
    fn strict_layout_render_rejects_unknown_variable_in_partial() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-strict-partial-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");
        let partial = partials.join("cabecalho.html");

        fs::write(&page, "<main>Conteúdo</main>").unwrap();
        fs::write(&layout, "<html>{{> cabecalho}}{{{conteudo}}}</html>").unwrap();
        fs::write(&partial, "<h1>{{titulo_inexistente}}</h1>").unwrap();

        let context = TemplateContext::new();

        let result = context.render_with_layout_strict(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
        );

        assert!(matches!(
            result,
            Err(TemplateError::UndefinedVariable(name))
                if name == "titulo_inexistente"
        ));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn strict_render_rejects_unknown_variable_in_if_condition() {
        let context = TemplateContext::new();

        let result =
            context.render_strict("{{#if utilizador_inexistente}}Olá{{else}}Visitante{{/if}}");

        assert!(matches!(
            result,
            Err(TemplateError::UndefinedVariable(name))
                if name == "utilizador_inexistente"
        ));
    }

    #[test]
    fn strict_render_rejects_unknown_variable_in_nested_if() {
        let mut context = TemplateContext::new();
        context.set("autenticado", serde_json::Value::Bool(true));

        let result = context.render_strict(
            "{{#if autenticado}}\
            {{#if permissao_inexistente}}\
                Acesso permitido\
            {{/if}}\
        {{/if}}",
        );

        assert!(matches!(
            result,
            Err(TemplateError::UndefinedVariable(name))
                if name == "permissao_inexistente"
        ));
    }

    #[test]
    fn template_validation_rejects_unclosed_if() {
        let result = TemplateContext::validate_template("{{#if utilizador}}Olá");

        assert!(result.is_err());
    }

    #[test]
    fn template_validation_rejects_unclosed_each() {
        let result = TemplateContext::validate_template("{{#each produtos}}<p>{{nome}}</p>");

        assert!(result.is_err());
    }

    #[test]
    fn validation_rejects_unclosed_if() {
        let result = TemplateContext::validate_template("{{#if utilizador}}Olá");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_unclosed_each() {
        let result = TemplateContext::validate_template("{{#each produtos}}<p>{{nome}}</p>");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_mismatched_closing_block() {
        let result = TemplateContext::validate_template("{{#if utilizador}}Olá{{/each}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_accepts_nested_blocks() {
        let result = TemplateContext::validate_template(
            "{{#if autenticado}}{{#each produtos}}{{nome}}{{/each}}{{/if}}",
        );

        assert!(result.is_ok());
    }

    #[test]
    fn strict_layout_render_rejects_invalid_syntax_in_partial() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-invalid-partial-syntax-{unique}"));
        let partials = base.join("partials");

        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");
        let partial = partials.join("cabecalho.html");

        fs::write(&page, "<main>Conteúdo</main>").unwrap();
        fs::write(
            &layout,
            "<html><body>{{> cabecalho}} {{{conteudo}}}</body></html>",
        )
        .unwrap();

        // O partial contém um bloco if sem fecho.
        fs::write(&partial, "{{#if titulo}}<h1>Olá</h1>").unwrap();

        let context = TemplateContext::new();

        let result = context.render_with_layout_strict(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
        );

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe, mas recebeu: {result:?}"
        );

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn validation_rejects_duplicate_else() {
        let template = "{{#if autenticado}}Bem-vindo{{else}}Negado{{else}}Inválido{{/if}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para um else duplicado."
        );
    }

    #[test]
    fn validation_rejects_else_inside_each() {
        let template = "{{#each alunos}}Nome: {{this}}{{else}}Sem alunos{{/each}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para else dentro de each."
        );
    }

    #[test]
    fn validation_rejects_if_without_condition() {
        let template = "{{#if}}Bem-vindo{{/if}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para if sem condição."
        );
    }

    #[test]
    fn validation_rejects_each_without_collection() {
        let template = "{{#each}}<p>{{this}}</p>{{/each}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para each sem coleção."
        );
    }

    #[test]
    fn validation_rejects_invalid_each_collection_name() {
        let template = "{{#each alunos-lista}}<p>{{this}}</p>{{/each}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para nome de coleção inválido."
        );
    }

    #[test]
    fn validation_rejects_invalid_if_condition_name() {
        let template = "{{#if utilizador-ativo}}Bem-vindo{{/if}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para nome de condição inválido."
        );
    }

    #[test]
    fn validation_rejects_expression_without_closing_delimiter() {
        let template = "Olá, {{utilizador";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para uma expressão sem delimitador de fecho."
        );
    }

    #[test]
    fn validation_rejects_empty_expression() {
        let template = "Olá, {{}}";

        let result = TemplateContext::validate_template(template);

        assert!(
            matches!(result, Err(TemplateError::Syntax(_))),
            "Esperava um erro de sintaxe para uma expressão vazia."
        );
    }

    #[test]
    fn validation_rejects_unsupported_block_expression() {
        let result = TemplateContext::validate_template("{{#unless ativo}}Olá{{/unless}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_invalid_variable_expression() {
        let result = TemplateContext::validate_template("Olá, {{nome-completo}}!");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_malformed_else_expression() {
        let result =
            TemplateContext::validate_template("{{#if ativo}}Olá{{else nome}}Mundo{{/if}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_invalid_variable_inside_if() {
        let result =
            TemplateContext::validate_template("{{#if utilizador}}Olá, {{nome-completo}}!{{/if}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_invalid_variable_inside_each() {
        let result =
            TemplateContext::validate_template("{{#each alunos}}{{nome-completo}}{{/each}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn normal_render_preserves_unclosed_if_as_literal_text() {
        let mut context = TemplateContext::new();
        context.set("ativo", serde_json::json!(true));

        let template = "{{#if ativo}}Conteúdo";

        let result = context.render(template);

        assert_eq!(result, "{{#if ativo}}Conteúdo");
    }

    #[test]
    fn strict_render_rejects_unclosed_if() {
        let mut context = TemplateContext::new();
        context.set("ativo", serde_json::json!(true));

        let template = "{{#if ativo}}Conteúdo";

        let result = context.render_strict(template);

        println!("Resultado do render estrito: {result:?}");

        assert!(result.is_err());
    }

    #[test]
    fn normal_render_limits_nested_each_depth() {
        let mut context = TemplateContext::new();

        context.set(
            "itens",
            serde_json::json!([
                {"nome": "Item A"}
            ]),
        );

        let mut template = String::new();

        for _ in 0..25 {
            template.push_str("{{#each itens}}");
        }

        template.push_str("Conteúdo");

        for _ in 0..25 {
            template.push_str("{{/each}}");
        }

        let result = context.render(&template);

        assert!(result.contains("{{#each itens}}"));
        assert!(result.contains("Conteúdo"));
        assert!(result.contains("{{/each}}"));
    }

    #[test]
    fn normal_render_preserves_each_after_depth_limit() {
        let mut context = TemplateContext::new();

        context.set(
            "itens",
            serde_json::json!([
                {"nome": "Item A"}
            ]),
        );

        let mut template = String::new();

        for _ in 0..25 {
            template.push_str("{{#each itens}}");
        }

        template.push_str("{{nome}}");

        for _ in 0..25 {
            template.push_str("{{/each}}");
        }

        let result = context.render(&template);

        let remaining_blocks = result.matches("{{#each itens}}").count();

        assert_eq!(
            remaining_blocks, 4,
            "O número de blocos não processados foi inesperado. Resultado: {result}"
        );

        assert!(result.contains("Item A"));
    }

    #[test]
    fn normal_render_supports_each_containing_if() {
        let mut context = TemplateContext::new();

        context.set(
            "itens",
            serde_json::json!([
                {"nome": "Item A"},
                {"nome": "Item B"}
            ]),
        );
        context.set("ativo", serde_json::json!(true));

        let template = "{{#each itens}}{{#if ativo}}{{nome}};{{/if}}{{/each}}";
        let result = context.render(template);

        assert_eq!(result, "Item A;Item B;");
    }

    #[test]
    fn normal_render_supports_if_containing_each() {
        let mut context = TemplateContext::new();

        context.set(
            "itens",
            serde_json::json!([
                {"nome": "Item A"},
                {"nome": "Item B"}
            ]),
        );
        context.set("ativo", serde_json::json!(true));

        let template = "{{#if ativo}}{{#each itens}}{{nome}};{{/each}}{{/if}}";
        let result = context.render(template);

        assert_eq!(result, "Item A;Item B;");
    }

    #[test]
    fn strict_render_rejects_excessive_mixed_nesting() {
        let mut context = TemplateContext::new();

        context.set(
            "itens",
            serde_json::json!([
                {"nome": "Item A"}
            ]),
        );
        context.set("ativo", serde_json::json!(true));

        let mut template = String::new();

        for _ in 0..12 {
            template.push_str("{{#each itens}}");
            template.push_str("{{#if ativo}}");
        }

        template.push_str("{{nome}}");

        for _ in 0..12 {
            template.push_str("{{/if}}");
            template.push_str("{{/each}}");
        }

        let result = context.render_strict(&template);

        assert!(
            matches!(result, Err(TemplateError::NestingLimitExceeded)),
            "Esperava NestingLimitExceeded, mas recebi: {result:?}"
        );
    }

    #[test]
    fn validation_rejects_if_closing_without_open_block() {
        let result = TemplateContext::validate_template("{{/if}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_each_closing_without_open_block() {
        let result = TemplateContext::validate_template("{{/each}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn validation_rejects_if_closing_each_block() {
        let result = TemplateContext::validate_template("{{#each produtos}}Olá{{/if}}");

        assert!(matches!(result, Err(TemplateError::Syntax(_))));
    }

    #[test]
    fn variable_values_are_not_reinterpreted_inside_if() {
        let mut context = TemplateContext::new();
        context.set("ativo", true.into());
        context.set("mensagem", "{{segredo}}".into());
        context.set("segredo", "VALOR_PRIVADO".into());

        let result = context
            .render_strict("{{#if ativo}}{{mensagem}}{{/if}}")
            .unwrap();

        assert_eq!(result, "{{segredo}}");
    }

    #[test]
    fn variable_values_are_not_reinterpreted_inside_each() {
        let mut context = TemplateContext::new();

        context.set(
            "mensagens",
            vec!["{{segredo}}", "<script>alert(1)</script>"].into(),
        );
        context.set("segredo", "VALOR_PRIVADO".into());

        let result = context
            .render_strict("{{#each mensagens}}{{this}}|{{/each}}")
            .unwrap();

        assert_eq!(result, "{{segredo}}|&lt;script&gt;alert(1)&lt;/script&gt;|");
    }

    #[test]
    fn each_preserves_template_expressions_inside_string_values() {
        let mut context = TemplateContext::new();

        context.set("mensagens", vec!["{{segredo}}", "{{outro_segredo}}"].into());
        context.set("segredo", "PRIVADO_1".into());
        context.set("outro_segredo", "PRIVADO_2".into());

        let result = context
            .render_strict("{{#each mensagens}}{{this}}|{{/each}}")
            .unwrap();

        assert_eq!(result, "{{segredo}}|{{outro_segredo}}|");
    }
}
