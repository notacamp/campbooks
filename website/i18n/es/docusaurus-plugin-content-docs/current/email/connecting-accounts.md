---
title: "Conectar cuentas de correo"
description: "Cómo conectar cuentas de correo a Campbooks mediante OAuth"
sidebar_position: 1
---

Campbooks se conecta a tu proveedor de correo mediante OAuth. Es compatible con Zoho Mail, Google Workspace y Microsoft 365.

## Proveedores compatibles

| Proveedor | Protocolo | Configuración |
|-----------|-----------|---------------|
| Zoho Mail | REST API | Zoho Developer Console |
| Google Workspace | Gmail API | Google Cloud Console |
| Microsoft 365 | Microsoft Graph | Registro de aplicación en Azure |

## Zoho Mail

### 1. Crear un cliente API de Zoho

1. Ve a la [Zoho Developer Console](https://api-console.zoho.com/)
2. Crea una nueva **Aplicación basada en servidor**
3. Añade el URI de redirección: `https://your-domain.com/oauth/zoho/callback`
4. Anota tu **Client ID** y **Client Secret**

### 2. Configurar las variables de entorno

```bash
ZOHO_CLIENT_ID=your_client_id
ZOHO_CLIENT_SECRET=your_client_secret
```

### 3. Conectar en Campbooks

1. Ve a Ajustes → Cuentas de correo
2. Haz clic en "Añadir cuenta de correo"
3. Selecciona Zoho Mail
4. Serás redirigido a Zoho para autorizar la conexión
5. Tras la autorización, Campbooks almacena un token de actualización cifrado

## Google Workspace

### 1. Crear un proyecto en Google Cloud

1. Ve a la [Google Cloud Console](https://console.cloud.google.com/)
2. Crea un nuevo proyecto
3. Activa la **Gmail API**
4. Crea credenciales OAuth 2.0 (Aplicación web)
5. Añade el URI de redirección: `https://your-domain.com/oauth/google/callback`

### 2. Configurar las variables de entorno

```bash
GOOGLE_CLIENT_ID=your_client_id
GOOGLE_CLIENT_SECRET=your_client_secret
```

### 3. Conectar en Campbooks

1. Ve a Ajustes → Cuentas de correo
2. Haz clic en "Añadir cuenta de correo"
3. Selecciona Google Workspace
4. Autoriza mediante la pantalla de consentimiento OAuth de Google

## Microsoft 365

### 1. Registrar una aplicación en Azure

1. Ve al [Portal de Azure](https://portal.azure.com/) → Registros de aplicaciones
2. Registra una nueva aplicación
3. Añade el URI de redirección: `https://your-domain.com/oauth/microsoft/callback`
4. Genera un secreto de cliente

### 2. Configurar las variables de entorno

```bash
MICROSOFT_CLIENT_ID=your_client_id
MICROSOFT_CLIENT_SECRET=your_client_secret
```

## Escaneo de correo

Una vez conectada la cuenta, Campbooks escanea tu bandeja de entrada automáticamente. Puedes:

- **Lanzar un escaneo manualmente** desde la página de Escaneos de correo
- **Programar escaneos recurrentes** mediante las tareas recurrentes de Solid Queue
- **Filtrar por carpeta** (Bandeja de entrada, Enviados, etc.)

Cada escaneo obtiene los mensajes a través de la API del proveedor y descarga los adjuntos. Consulta [Escaneo de correo](/docs/email/scanning) para más detalles.
