---
title: "Self-Hosting Guide"
description: "Step-by-step guide to self-hosting Campbooks"
sidebar_position: 2
---

Self-host Campbooks on your own server. This guide covers a production setup with Nginx, SSL, and systemd.

<div class="callout callout-note">
  **Estimated time:** 20–30 minutes for a fresh Ubuntu server. All commands assume you're logged in as a user with `sudo` access.
</div>

## Server requirements

- **OS**: Ubuntu 24.04 LTS (or any Linux with a recent Ruby)
- **CPU**: 2 cores minimum (4 recommended for AI features)
- **RAM**: 2 GB minimum (4 GB recommended)
- **Storage**: 20 GB minimum (more for email attachments)

## Step 1: Install dependencies

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

## Step 2: Set up PostgreSQL

```bash
sudo -u postgres createuser campbooks --createdb --pwprompt
sudo -u postgres createdb campbooks_production -O campbooks
```

## Step 3: Clone and configure

```bash
git clone https://github.com/notacamp/campbooks.git /opt/campbooks
cd /opt/campbooks
bundle config set --local deployment 'true'
bundle install
```

Create `/opt/campbooks/.env`:

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

## Step 4: Set up the database

```bash
RAILS_ENV=production bin/rails db:migrate
RAILS_ENV=production bin/rails db:seed
RAILS_ENV=production bin/rails assets:precompile
```

<div class="callout callout-warning">
  **Don't skip this step.** Running Rails directly from the terminal will work for testing but won't survive reboots or crashes. systemd keeps the app running reliably.
</div>

## Step 5: Configure systemd

Create `/etc/systemd/system/campbooks-web.service`:

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

Create `/etc/systemd/system/campbooks-worker.service`:

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

Enable and start:

```bash
sudo systemctl enable --now campbooks-web campbooks-worker
```

## Step 6: Configure Nginx

Create `/etc/nginx/sites-available/campbooks`:

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

Enable:

```bash
sudo ln -s /etc/nginx/sites-available/campbooks /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

## Step 7: Set up SSL

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com
```

## Maintenance

<div class="callout callout-info">
  **Routine upkeep.** Campbooks uses Solid Queue for background jobs — no Redis to maintain. Backups are just PostgreSQL dumps. The entire maintenance surface is small.
</div>

- **Updates**: `git pull && bundle install && RAILS_ENV=production bin/rails db:migrate && sudo systemctl restart campbooks-web campbooks-worker`
- **Logs**: `journalctl -u campbooks-web -f`
- **Backups**: `pg_dump campbooks_production > backup.sql`
