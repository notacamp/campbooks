---
title: "Descripción general del despliegue"
description: "Cómo desplegar Campbooks en producción"
sidebar_position: 1
---

Despliega Campbooks en tu propia infraestructura. Es una aplicación Rails estándar y puede ejecutarse en cualquier servidor que soporte Ruby y PostgreSQL.

## Opciones de despliegue

- **Kamal** — despliega en cualquier VPS con Docker
- **Docker Compose** — ejecútalo en un único servidor
- **Heroku / Render** — plataforma como servicio
- **Servidor dedicado** — ejecútalo directamente sobre el sistema operativo

## Stack recomendado

Para un despliegue en producción:

- **Servidor web**: Puma (incluido con Rails)
- **Tareas en segundo plano**: Solid Queue (respaldado por base de datos, sin necesidad de Redis)
- **Base de datos**: PostgreSQL 16+
- **Almacenamiento**: Disco local o compatible con S3 (AWS S3, MinIO, Cloudflare R2)
- **Proxy inverso**: Nginx o Caddy
- **SSL**: Let's Encrypt mediante Caddy o Certbot

## Variables de entorno

Toda la configuración se realiza mediante variables de entorno. Las principales son:

| Variable | Obligatoria | Propósito |
|----------|----------|---------|
| `DATABASE_URL` | Sí | Cadena de conexión a PostgreSQL |
| `RAILS_ENV` | Sí | Establecer en `production` |
| `SECRET_KEY_BASE` | Sí | Clave secreta de Rails |
| `ACTIVE_RECORD_PRIMARY_KEY` | Sí | Clave de cifrado |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Sí | Clave de cifrado |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Sí | Sal de cifrado |

Genera `SECRET_KEY_BASE`:

```bash
bin/rails secret
```

Genera las claves de cifrado:

```bash
bin/rails db:encryption:init
```

## Precompilar assets

Antes de desplegar, precompila los assets:

```bash
RAILS_ENV=production bin/rails assets:precompile
```

## Ejecutar en producción

```bash
RAILS_ENV=production bin/rails server
RAILS_ENV=production bin/rails solid_queue:start
```

O usa el Procfile con un gestor de procesos como systemd o supervisor.

## Comprobación de estado

Campbooks incluye un endpoint de comprobación de estado en `/up`. Úsalo para monitorización y comprobaciones de estado del balanceador de carga.
