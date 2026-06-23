---
title: "Instalação"
description: "Como instalar e executar o Campbooks no seu próprio servidor"
sidebar_position: 2
---

Instale o Campbooks no seu próprio servidor. Vai precisar de Ruby 3.3+, PostgreSQL 16+ e Node.js 18+.

<div class="callout callout-note">
  **Novo em Rails?** O Campbooks é uma aplicação Rails padrão. Se já fez deploy de aplicações Rails antes, este processo vai parecer familiar. A maioria dos passos segue as convenções do Rails.
</div>

## Pré-requisitos

- **Ruby** 3.3 ou posterior
- **PostgreSQL** 16 ou posterior
- **Node.js** 18 ou posterior
- **Redis** (para Action Cable, opcional — usa Solid Cable por defeito)
- **OpenSearch** (para pesquisa de texto integral, opcional — usa PostgreSQL por defeito)

## Clonar o repositório

```bash
git clone https://github.com/notacamp/campbooks.git
cd campbooks
```

## Instalar as dependências

```bash
bundle install
```

## Configurar a base de dados

```bash
bin/rails db:create
bin/rails db:migrate
bin/rails db:seed
```

<div class="callout callout-info">
  **Utilizadores de seed.** O comando seed cria duas contas para testes:
  `admin@example.com` e `partner@example.com`, ambas com a palavra-passe `changeme123`.
</div>

## Configurar variáveis de ambiente

Copie o ficheiro de ambiente de exemplo e preencha os seus valores:

```bash
cp .env.example .env
```

**Obrigatórias:**

| Variável | Finalidade |
|----------|---------|
| `DATABASE_URL` | String de ligação ao PostgreSQL |
| `ACTIVE_RECORD_PRIMARY_KEY` | Chave primária de encriptação |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Chave determinística de encriptação |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Salt de derivação da chave de encriptação |

**Opcionais mas recomendadas:**

| Variável | Finalidade |
|----------|---------|
| `ANTHROPIC_API_KEY` | Chave da API Claude para funcionalidades de IA |
| `OPENAI_API_KEY` | Chave da API OpenAI para embeddings |
| `ZOHO_CLIENT_ID` / `ZOHO_CLIENT_SECRET` | Credenciais OAuth do Zoho Mail |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | Credenciais OAuth do Google |

<div class="callout callout-warning">
  **As chaves de encriptação são obrigatórias.** Sem `ACTIVE_RECORD_PRIMARY_KEY`, `ACTIVE_RECORD_DETERMINISTIC_KEY` e `ACTIVE_RECORD_KEY_DERIVATION_SALT`, a aplicação não irá arrancar. Gere-as com `bin/rails secret` e utilize o resultado para cada chave.
</div>

Gerar chaves de encriptação:

```bash
bin/rails secret
```

## Iniciar a aplicação

```bash
bin/rails server               # Servidor web na porta 3000
bin/rails solid_queue:start    # Worker de tarefas em segundo plano
```

Ou com o Procfile:

```bash
bin/dev
```

Abra `http://localhost:3000` e inicie sessão com um dos utilizadores de seed.

## Docker

É fornecido um Dockerfile para deploys em produção:

```bash
docker build -t campbooks .
docker run -p 3000:3000 --env-file .env campbooks
```

<div class="callout callout-note">
  **Próximo passo.** Consulte o [guia de Implementação](/docs/deployment/overview) para uma configuração completa de produção com Nginx, SSL e systemd.
</div>
