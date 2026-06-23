---
title: "Visão Geral da Implementação"
description: "Como fazer deploy do Campbooks em produção"
sidebar_position: 1
---

Faça deploy do Campbooks na sua própria infraestrutura. É uma aplicação Rails padrão e pode correr em qualquer servidor que suporte Ruby e PostgreSQL.

## Opções de implementação

- **Kamal** — deploy em qualquer VPS com Docker
- **Docker Compose** — executar num servidor único
- **Heroku / Render** — plataforma como serviço
- **Bare metal** — executar diretamente num servidor

## Stack recomendado

Para um deploy em produção:

- **Servidor web**: Puma (incluído no Rails)
- **Tarefas em segundo plano**: Solid Queue (baseado em base de dados, sem necessidade de Redis)
- **Base de dados**: PostgreSQL 16+
- **Armazenamento**: Disco local ou compatível com S3 (AWS S3, MinIO, Cloudflare R2)
- **Proxy inverso**: Nginx ou Caddy
- **SSL**: Let's Encrypt via Caddy ou Certbot

## Variáveis de ambiente

Toda a configuração é feita através de variáveis de ambiente. As principais:

| Variável | Obrigatória | Finalidade |
|----------|----------|---------|
| `DATABASE_URL` | Sim | String de ligação ao PostgreSQL |
| `RAILS_ENV` | Sim | Definir como `production` |
| `SECRET_KEY_BASE` | Sim | Chave secreta do Rails |
| `ACTIVE_RECORD_PRIMARY_KEY` | Sim | Chave de encriptação |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Sim | Chave de encriptação |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Sim | Salt de encriptação |

Gerar `SECRET_KEY_BASE`:

```bash
bin/rails secret
```

Gerar chaves de encriptação:

```bash
bin/rails db:encryption:init
```

## Pré-compilar assets

Antes de fazer deploy, pré-compile os assets:

```bash
RAILS_ENV=production bin/rails assets:precompile
```

## Executar em produção

```bash
RAILS_ENV=production bin/rails server
RAILS_ENV=production bin/rails solid_queue:start
```

Ou utilize o Procfile com um gestor de processos como o systemd ou supervisor.

## Verificação de estado

O Campbooks inclui um endpoint de verificação de estado em `/up`. Utilize-o para monitorização e verificações de estado do balanceador de carga.
