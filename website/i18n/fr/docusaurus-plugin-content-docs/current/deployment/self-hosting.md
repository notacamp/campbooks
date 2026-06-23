---
title: "Guide d'auto-hébergement"
description: "Guide pas à pas pour auto-héberger Campbooks"
sidebar_position: 2
---

Auto-hébergez Campbooks sur votre propre serveur. Ce guide couvre une configuration en production avec Nginx, SSL et systemd.

<div class="callout callout-note">
  **Durée estimée :** 20 à 30 minutes sur un serveur Ubuntu vierge. Toutes les commandes supposent que vous êtes connecté en tant qu'utilisateur disposant des droits `sudo`.
</div>

## Configuration requise du serveur

- **Système d'exploitation** : Ubuntu 24.04 LTS (ou tout Linux avec une version récente de Ruby)
- **CPU** : 2 cœurs minimum (4 recommandés pour les fonctionnalités IA)
- **RAM** : 2 Go minimum (4 Go recommandés)
- **Stockage** : 20 Go minimum (davantage pour les pièces jointes aux e-mails)

## Étape 1 : Installer les dépendances

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

## Étape 2 : Configurer PostgreSQL

```bash
sudo -u postgres createuser campbooks --createdb --pwprompt
sudo -u postgres createdb campbooks_production -O campbooks
```

## Étape 3 : Cloner et configurer

```bash
git clone https://github.com/notacamp/campbooks.git /opt/campbooks
cd /opt/campbooks
bundle config set --local deployment 'true'
bundle install
```

Créez `/opt/campbooks/.env` :

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

## Étape 4 : Configurer la base de données

```bash
RAILS_ENV=production bin/rails db:migrate
RAILS_ENV=production bin/rails db:seed
RAILS_ENV=production bin/rails assets:precompile
```

<div class="callout callout-warning">
  **Ne sautez pas cette étape.** Exécuter Rails directement depuis le terminal fonctionnera pour les tests, mais ne survivra pas aux redémarrages ou aux pannes. systemd maintient l'application en fonctionnement de manière fiable.
</div>

## Étape 5 : Configurer systemd

Créez `/etc/systemd/system/campbooks-web.service` :

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

Créez `/etc/systemd/system/campbooks-worker.service` :

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

Activez et démarrez :

```bash
sudo systemctl enable --now campbooks-web campbooks-worker
```

## Étape 6 : Configurer Nginx

Créez `/etc/nginx/sites-available/campbooks` :

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

Activez :

```bash
sudo ln -s /etc/nginx/sites-available/campbooks /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

## Étape 7 : Configurer SSL

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com
```

## Maintenance

<div class="callout callout-info">
  **Entretien courant.** Campbooks utilise Solid Queue pour les tâches en arrière-plan — pas de Redis à maintenir. Les sauvegardes se résument à des dumps PostgreSQL. La surface de maintenance globale est réduite.
</div>

- **Mises à jour** : `git pull && bundle install && RAILS_ENV=production bin/rails db:migrate && sudo systemctl restart campbooks-web campbooks-worker`
- **Journaux** : `journalctl -u campbooks-web -f`
- **Sauvegardes** : `pg_dump campbooks_production > backup.sql`
