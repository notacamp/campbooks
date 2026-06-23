---
title: "Guía de autoalojamiento"
description: "Guía paso a paso para alojar Campbooks en tu propio servidor"
sidebar_position: 2
---

Aloja Campbooks en tu propio servidor. Esta guía cubre una configuración en producción con Nginx, SSL y systemd.

<div class="callout callout-note">
  **Tiempo estimado:** 20–30 minutos en un servidor Ubuntu nuevo. Todos los comandos asumen que has iniciado sesión como un usuario con acceso `sudo`.
</div>

## Requisitos del servidor

- **SO**: Ubuntu 24.04 LTS (o cualquier Linux con una versión reciente de Ruby)
- **CPU**: 2 núcleos mínimo (4 recomendados para las funciones de IA)
- **RAM**: 2 GB mínimo (4 GB recomendados)
- **Almacenamiento**: 20 GB mínimo (más si hay muchos archivos adjuntos de correo)

## Paso 1: Instalar dependencias

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

## Paso 2: Configurar PostgreSQL

```bash
sudo -u postgres createuser campbooks --createdb --pwprompt
sudo -u postgres createdb campbooks_production -O campbooks
```

## Paso 3: Clonar y configurar

```bash
git clone https://github.com/notacamp/campbooks.git /opt/campbooks
cd /opt/campbooks
bundle config set --local deployment 'true'
bundle install
```

Crea `/opt/campbooks/.env`:

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

## Paso 4: Configurar la base de datos

```bash
RAILS_ENV=production bin/rails db:migrate
RAILS_ENV=production bin/rails db:seed
RAILS_ENV=production bin/rails assets:precompile
```

<div class="callout callout-warning">
  **No omitas este paso.** Ejecutar Rails directamente desde la terminal funcionará para pruebas, pero no sobrevivirá a reinicios ni fallos del sistema. systemd mantiene la aplicación en ejecución de forma fiable.
</div>

## Paso 5: Configurar systemd

Crea `/etc/systemd/system/campbooks-web.service`:

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

Crea `/etc/systemd/system/campbooks-worker.service`:

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

Habilita e inicia:

```bash
sudo systemctl enable --now campbooks-web campbooks-worker
```

## Paso 6: Configurar Nginx

Crea `/etc/nginx/sites-available/campbooks`:

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

Habilita:

```bash
sudo ln -s /etc/nginx/sites-available/campbooks /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

## Paso 7: Configurar SSL

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com
```

## Mantenimiento

<div class="callout callout-info">
  **Tareas habituales.** Campbooks usa Solid Queue para las tareas en segundo plano — sin Redis que mantener. Las copias de seguridad son simplemente volcados de PostgreSQL. La superficie de mantenimiento es pequeña.
</div>

- **Actualizaciones**: `git pull && bundle install && RAILS_ENV=production bin/rails db:migrate && sudo systemctl restart campbooks-web campbooks-worker`
- **Registros**: `journalctl -u campbooks-web -f`
- **Copias de seguridad**: `pg_dump campbooks_production > backup.sql`
