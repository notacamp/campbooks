---
title: "Connecting Email Accounts"
description: "How to connect email accounts to Campbooks via OAuth"
sidebar_position: 1
---

Campbooks connects to your email provider via OAuth. It supports Zoho Mail, Google Workspace, and Microsoft 365.

## Supported providers

| Provider | Protocol | Setup |
|----------|----------|-------|
| Zoho Mail | REST API | Zoho Developer Console |
| Google Workspace | Gmail API | Google Cloud Console |
| Microsoft 365 | Microsoft Graph | Azure App Registration |

## Zoho Mail

### 1. Create a Zoho API client

1. Go to the [Zoho Developer Console](https://api-console.zoho.com/)
2. Create a new **Server-based Application**
3. Add the redirect URI: `https://your-domain.com/oauth/zoho/callback`
4. Note your **Client ID** and **Client Secret**

### 2. Configure environment variables

```bash
ZOHO_CLIENT_ID=your_client_id
ZOHO_CLIENT_SECRET=your_client_secret
```

### 3. Connect in Campbooks

1. Go to Settings → Email Accounts
2. Click "Add Email Account"
3. Select Zoho Mail
4. You'll be redirected to Zoho to authorize the connection
5. After authorization, Campbooks stores an encrypted refresh token

## Google Workspace

### 1. Create a Google Cloud project

1. Go to the [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project
3. Enable the **Gmail API**
4. Create OAuth 2.0 credentials (Web application)
5. Add the redirect URI: `https://your-domain.com/oauth/google/callback`

### 2. Configure environment variables

```bash
GOOGLE_CLIENT_ID=your_client_id
GOOGLE_CLIENT_SECRET=your_client_secret
```

### 3. Connect in Campbooks

1. Go to Settings → Email Accounts
2. Click "Add Email Account"
3. Select Google Workspace
4. Authorize through Google's OAuth consent screen

## Microsoft 365

### 1. Register an Azure application

1. Go to the [Azure Portal](https://portal.azure.com/) → App registrations
2. Register a new application
3. Add the redirect URI: `https://your-domain.com/oauth/microsoft/callback`
4. Generate a client secret

### 2. Configure environment variables

```bash
MICROSOFT_CLIENT_ID=your_client_id
MICROSOFT_CLIENT_SECRET=your_client_secret
```

## Email scanning

Once connected, Campbooks automatically scans your inbox. You can:

- **Manually trigger** a scan from the Email Scans page
- **Schedule recurring scans** via Solid Queue recurring tasks
- **Filter by folder** (Inbox, Sent, etc.)

Each scan fetches messages via the provider's API and downloads attachments. See [Email Scanning](/docs/email/scanning) for details.
