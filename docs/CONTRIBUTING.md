# Contribuir para o RustPress

Obrigado por considerar contribuir para o RustPress. Este documento resume a forma recomendada de colaborar no projecto.

## Requisitos

- Rust estável
- Cargo
- Git
- Editor ou IDE com suporte a Rust (VS Code + rust-analyzer recomendado)

## Configuração local

```bash
git https://github.com/fabriciomichael/rustpress-web.git
cd rustpress-web
cargo build
cargo test
```

## Fluxo de trabalho recomendado

1. Crie uma branch para a funcionalidade ou correção.
2. Faça alterações pequenas e focadas.
3. Atualize a documentação sempre que a API mudar.
4. Execute testes e validações antes de submeter.
5. Abra um pull request com uma descrição objetiva.

## Normas de código

- Mantenha a API simples e consistente com o estilo do Rust.
- Prefira nomes claros e expressivos.
- Evite complexidade desnecessária.
- Documente funções públicas com comentários curtos e úteis.
- Não introduza dependências desnecessárias.

## Testes

Antes de submeter uma alteração:

```bash
cargo test
```

Se estiver a desenvolver novos exemplos:

```bash
cargo run --example basic
```

## Pull requests

Os pull requests devem incluir:

- descrição do problema ou funcionalidade;
- resumo das alterações;
- qualquer impacto de compatibilidade;
- instruções de validação.

## Boas práticas

- Mantenha o foco no problema.
- Descreva claramente mudanças de comportamento.
- Documente qualquer mudança na API pública.
- Use commits pequenos e semânticos.

## Suporte e discussão

Use as issues e pull requests do repositório para propor melhorias, reportar bugs e discutir alterações de arquitetura.
