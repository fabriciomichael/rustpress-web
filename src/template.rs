use serde_json::{Map, Value};

/// Contexto de dados disponibilizado ao template.
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

    /// Renderiza variáveis e condições de um template.

    pub fn render(&self, template: &str) -> String {
        let result = self.render_each(template);
        let result = self.render_conditions(&result);
        self.render_variables(&result)
    }

    /// Interpreta blocos if, else e /if, incluindo condições aninhadas.
    fn render_conditions(&self, template: &str) -> String {
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

            output.push_str(&self.render_conditions(selected));
            position = close_end;
        }

        output.push_str(&template[position..]);
        output
    }

    /// Substitui variáveis e escapa os respectivos valores.
    fn render_variables(&self, template: &str) -> String {
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

            if let Some(value) = self.get(key) {
                output.push_str(&escape_html(&value_to_string(value)));
            } else {
                // Mantém expressões desconhecidas para facilitar
                // a identificação de erros no template.
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

    fn render_each(&self, template: &str) -> String {
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

                    // As propriedades do item ficam disponíveis
                    // directamente no template: {{nome}}, {{preco}}.
                    if let Value::Object(properties) = item {
                        for (name, value) in properties {
                            child.set(name.clone(), value.clone());
                        }
                    } else {
                        child.set("this", item.clone());
                    }

                    output.push_str(&child.render(body));
                }
            }

            position = close_end;
        }

        output.push_str(&template[position..]);
        output
    }

    /// Compõe uma página usando um layout e partials reutilizáveis.
    pub fn render_with_layout(
        &self,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
    ) -> std::io::Result<String> {
        // 1. Ler o conteúdo específico da página.
        let page = std::fs::read_to_string(page_path)?;

        // 2. Ler o layout principal.
        let layout = std::fs::read_to_string(layout_path)?;

        // 3. Inserir o conteúdo da página no layout.
        let combined = layout.replace("{{{conteudo}}}", &page);

        // 4. Processar os partials.
        let combined = expand_partials(&combined, partials_dir)?;

        // 5. Renderizar variáveis, condições e ciclos.
        Ok(self.render(&combined))
    }
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
    let mut result = template.to_string();

    // Limita a recursividade para evitar inclusões infinitas.
    for _ in 0..20 {
        let Some(start) = result.find("{{>") else {
            return Ok(result);
        };

        let Some(relative_end) = result[start + 3..].find("}}") else {
            return Ok(result);
        };

        let end = start + 3 + relative_end;
        let name = result[start + 3..end].trim();

        // Aceitar apenas nomes simples de partials.
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

        let path = std::path::Path::new(partials_dir).join(format!("{name}.html"));

        let partial = std::fs::read_to_string(path)?;

        // Expandir também os partials incluídos noutros partials.
        let partial = expand_partials(&partial, partials_dir)?;

        result.replace_range(start..end + 2, &partial);
    }

    if result.contains("{{>") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Limite de inclusões de partials excedido",
        ));
    }

    Ok(result)
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
}
