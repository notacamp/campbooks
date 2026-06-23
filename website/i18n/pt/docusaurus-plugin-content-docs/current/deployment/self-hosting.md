---
title: "Guia de Auto-Alojamento"
description: "Guia passo a passo para auto-alojar o Campbooks"
sidebar_position: 2
---

Auto-aloje o Campbooks no seu próprio servidor. Este guia cobre uma configuração de produção com Nginx, SSL e systemd.

<div class="callout callout-note">
  **Tempo estimado:** 20–30 minutos para um servidor Ubuntu novo. Todos os comandos assumem que está autenticado como um utilizador com acesso `sudo`.
</div>

## Requisitos do servidor

- **SO**: Ubuntu 24.04 LTS (ou qualquer Linux com uma versão recente de Ruby)
- **CPU**: mínimo 2 núcleos (4 recomendados para funcionalidades de IA)
- **RAM**: mínimo 2 GB (4 GB recomendados)
- **Armazenamento**: mínimo 20 GB (mais para anexos de email)

## Passo 1: Instalar as dependências

```bash
sudo apt update
sudo apt install -y \
  ruby-full \
  postgresql \
  nodejs \
  nginx \
  git \
  build-essential
```

## Passo 2: Configurar o PostgreSQL

```bash
sudo -u postgres createuser campbooks --createdb --pwprompt
sudo -u postgres createdb campbooks_production -O campbooks
```

## Passo 3: Clonar e configurar

```bash
git clone https://github.com/notacamp/campbooks.git /opt/campbooks
cd /opt/campbooks
bundle config set --local deployment 'true'
bundle install
```

Criar `/opt/campbooks/.env`:

```bash
DATABASE_URL=postgresql://campbooks:password@localhost/campbooks_production
RAILS_ENV=production
SECRET_KEY_BASE=<generated_secret>
ACTIVE_RECORD_PRIMARY_KEY=<generated_key>
ACTIVE_RECORD_DETERMINISTIC_KEY=<generated_key>
ACTIVE_RECORD_KEY_DERIVATION_SALT=<generated_salt>
ANTHROPIC_API_KEY=<your_api_key>
RAILS_SERVE_STATIC_FILES=true
```

## Passo 4: Configurar a base de dados

```bash
RAILS_ENV=production bin/rails db:migrate
RAILS_ENV=production bin/rails db:seed
RAILS_ENV=production bin/rails assets:precompile
```

<div class="callout callout-warning">
  **Não salte este passo.** Executar o Rails diretamente a partir do terminal funcionará para testes, mas não sobreviverá a reinicializações ou falhas. O systemd mantém a aplicação a correr de forma fiável.
</div>

## Passo 5: Configurar o systemd

Criar `/etc/systemd/system/campbooks-web.service`:

```ini
[Unit]
Description=Campbooks web server
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/opt/campbooks
EnvironmentFile=/opt/campbooks/.env
ExecStart=/usr/bin/bundle exec puma -C config/puma.rb
Restart=always

[Install]
WantedBy=multi-user.target
```

Criar `/etc/systemd/system/campbooks-worker.service`:

```ini
[Unit]
Description=Campbooks worker
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/opt/campbooks
EnvironmentFile=/opt/campbooks/.env
ExecStart=/usr/bin/bundle exec solid_queue:start
Restart=always

[Install]
WantedBy=multi-user.target
```

Ativar e iniciar:

```bash
sudo systemctl enable --now campbooks-web campbooks-worker
```

## Passo 6: Configurar o Nginx

Criar `/etc/nginx/sites-available/campbooks`:

```nginx
upstream campbooks {
    server 127.0.0.1:3000;
}

server {
    listen 80;
    server_name your-domain.com;

    location / {
        proxy_pass http://campbooks;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Ativar:

```bash
sudo ln -s /etc/nginx/sites-available/campbooks /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

## Passo 7: Configurar SSL

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com
```

## Manutenção

<div class="callout callout-info">
  **Manutenção de rotina.** O Campbooks usa o Solid Queue para tarefas em segundo plano — sem Redis para manter. As cópias de segurança são apenas dumps do PostgreSQL. A superfície de manutenção total é pequena.
</div>

- **Atualizações**: `git pull && bundle install && RAILS_ENV=production bin/rails db:migrate && sudo systemctl restart campbooks-web campbooks-worker`
- **Registos**: `journalctl -u campbooks-web -f`
- **Cópias de segurança**: `pg_dump campbooks_production > backup.sql`
