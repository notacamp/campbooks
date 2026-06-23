---
title: "Instalación"
description: "Cómo instalar y ejecutar Campbooks en tu propio servidor"
sidebar_position: 2
---

Instala Campbooks en tu propio servidor. Necesitarás Ruby 3.3+, PostgreSQL 16+ y Node.js 18+.

<div class="callout callout-note">
  **¿Nuevo en Rails?** Campbooks es una aplicación Rails estándar. Si ya has desplegado Rails antes, esto te resultará familiar. La mayoría de los pasos siguen las convenciones de Rails.
</div>

## Requisitos previos

- **Ruby** 3.3 o superior
- **PostgreSQL** 16 o superior
- **Node.js** 18 o superior
- **Redis** (para Action Cable, opcional — usa Solid Cable por defecto)
- **OpenSearch** (para búsqueda de texto completo, opcional — usa PostgreSQL por defecto)

## Clonar el repositorio

```bash
git clone https://github.com/notacamp/campbooks.git
cd campbooks
```

## Instalar dependencias

```bash
bundle install
```

## Configurar la base de datos

```bash
bin/rails db:create
bin/rails db:migrate
bin/rails db:seed
```

<div class="callout callout-info">
  **Usuarios de ejemplo.** El comando seed crea dos cuentas para pruebas:
  `admin@example.com` y `partner@example.com`, ambas con la contraseña `changeme123`.
</div>

## Configurar las variables de entorno

Copia el archivo de entorno de ejemplo y completa tus valores:

```bash
cp .env.example .env
```

**Obligatorias:**

| Variable | Propósito |
|----------|---------|
| `DATABASE_URL` | Cadena de conexión a PostgreSQL |
| `ACTIVE_RECORD_PRIMARY_KEY` | Clave primaria de cifrado |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Clave determinista de cifrado |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Sal de derivación de clave de cifrado |

**Opcionales pero recomendadas:**

| Variable | Propósito |
|----------|---------|
| `ANTHROPIC_API_KEY` | Clave de API de Claude para las funciones de IA |
| `OPENAI_API_KEY` | Clave de API de OpenAI para embeddings |
| `ZOHO_CLIENT_ID` / `ZOHO_CLIENT_SECRET` | Credenciales OAuth de Zoho Mail |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | Credenciales OAuth de Google |

<div class="callout callout-warning">
  **Las claves de cifrado son obligatorias.** Sin `ACTIVE_RECORD_PRIMARY_KEY`, `ACTIVE_RECORD_DETERMINISTIC_KEY` y `ACTIVE_RECORD_KEY_DERIVATION_SALT`, la aplicación no arrancará. Genéralas con `bin/rails secret` y usa el resultado para cada clave.
</div>

Genera las claves de cifrado:

```bash
bin/rails secret
```

## Iniciar la aplicación

```bash
bin/rails server               # Servidor web en el puerto 3000
bin/rails solid_queue:start    # Worker de tareas en segundo plano
```

O con el Procfile:

```bash
bin/dev
```

Abre `http://localhost:3000` e inicia sesión con uno de los usuarios de ejemplo.

## Docker

Se incluye un Dockerfile para despliegues en producción:

```bash
docker build -t campbooks .
docker run -p 3000:3000 --env-file .env campbooks
```

<div class="callout callout-note">
  **Siguiente paso.** Consulta la [guía de despliegue](/docs/deployment/overview) para una configuración completa en producción con Nginx, SSL y systemd.
</div>
