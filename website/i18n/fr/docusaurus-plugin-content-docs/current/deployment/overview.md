---
title: "Vue d'ensemble du déploiement"
description: "Comment déployer Campbooks en production"
sidebar_position: 1
---

Déployez Campbooks sur votre propre infrastructure. Il s'agit d'une application Rails standard, qui peut s'exécuter sur tout serveur prenant en charge Ruby et PostgreSQL.

## Options de déploiement

- **Kamal** — déployez sur n'importe quel VPS avec Docker
- **Docker Compose** — exécutez sur un serveur unique
- **Heroku / Render** — plateforme en tant que service
- **Bare metal** — exécutez directement sur un serveur

## Stack recommandée

Pour un déploiement en production :

- **Serveur web** : Puma (intégré à Rails)
- **Tâches en arrière-plan** : Solid Queue (basé sur la base de données, sans Redis)
- **Base de données** : PostgreSQL 16+
- **Stockage** : Disque local ou compatible S3 (AWS S3, MinIO, Cloudflare R2)
- **Proxy inverse** : Nginx ou Caddy
- **SSL** : Let's Encrypt via Caddy ou Certbot

## Variables d'environnement

Toute la configuration se fait via des variables d'environnement. Les principales :

| Variable | Obligatoire | Rôle |
|----------|-------------|------|
| `DATABASE_URL` | Oui | Chaîne de connexion PostgreSQL |
| `RAILS_ENV` | Oui | Définir sur `production` |
| `SECRET_KEY_BASE` | Oui | Clé secrète Rails |
| `ACTIVE_RECORD_PRIMARY_KEY` | Oui | Clé de chiffrement |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Oui | Clé de chiffrement |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Oui | Sel de chiffrement |

Générer `SECRET_KEY_BASE` :

```bash
bin/rails secret
```

Générer les clés de chiffrement :

```bash
bin/rails db:encryption:init
```

## Précompilation des assets

Avant de déployer, précompilez les assets :

```bash
RAILS_ENV=production bin/rails assets:precompile
```

## Exécution en production

```bash
RAILS_ENV=production bin/rails server
RAILS_ENV=production bin/rails solid_queue:start
```

Ou utilisez le Procfile avec un gestionnaire de processus comme systemd ou supervisor.

## Vérification de l'état de santé

Campbooks inclut un point de terminaison de vérification de l'état de santé à `/up`. Utilisez-le pour la surveillance et les vérifications de santé des équilibreurs de charge.
