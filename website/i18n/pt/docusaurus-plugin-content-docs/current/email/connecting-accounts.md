---
title: "Ligar Contas de Email"
description: "Como ligar contas de email ao Campbooks via OAuth"
sidebar_position: 1
---

O Campbooks liga-se ao seu fornecedor de email via OAuth. Suporta Zoho Mail, Google Workspace e Microsoft 365.

## Fornecedores suportados

| Fornecedor | Protocolo | Configuração |
|------------|-----------|--------------|
| Zoho Mail | REST API | Zoho Developer Console |
| Google Workspace | Gmail API | Google Cloud Console |
| Microsoft 365 | Microsoft Graph | Azure App Registration |

## Zoho Mail

### 1. Criar um cliente Zoho API

1. Vá para a [Zoho Developer Console](https://api-console.zoho.com/)
2. Crie uma nova **Server-based Application**
3. Adicione o URI de redirecionamento: `https://your-domain.com/oauth/zoho/callback`
4. Anote o seu **Client ID** e **Client Secret**

### 2. Configurar variáveis de ambiente

```bash
ZOHO_CLIENT_ID=your_client_id
ZOHO_CLIENT_SECRET=your_client_secret
```

### 3. Ligar no Campbooks

1. Vá a Definições → Contas de Email
2. Clique em "Adicionar Conta de Email"
3. Selecione Zoho Mail
4. Será redirecionado para o Zoho para autorizar a ligação
5. Após a autorização, o Campbooks armazena um token de atualização encriptado

## Google Workspace

### 1. Criar um projeto Google Cloud

1. Vá para a [Google Cloud Console](https://console.cloud.google.com/)
2. Crie um novo projeto
3. Ative a **Gmail API**
4. Crie credenciais OAuth 2.0 (aplicação Web)
5. Adicione o URI de redirecionamento: `https://your-domain.com/oauth/google/callback`

### 2. Configurar variáveis de ambiente

```bash
GOOGLE_CLIENT_ID=your_client_id
GOOGLE_CLIENT_SECRET=your_client_secret
```

### 3. Ligar no Campbooks

1. Vá a Definições → Contas de Email
2. Clique em "Adicionar Conta de Email"
3. Selecione Google Workspace
4. Autorize através do ecrã de consentimento OAuth do Google

## Microsoft 365

### 1. Registar uma aplicação Azure

1. Vá para o [Portal Azure](https://portal.azure.com/) → Registos de aplicações
2. Registe uma nova aplicação
3. Adicione o URI de redirecionamento: `https://your-domain.com/oauth/microsoft/callback`
4. Gere um segredo de cliente

### 2. Configurar variáveis de ambiente

```bash
MICROSOFT_CLIENT_ID=your_client_id
MICROSOFT_CLIENT_SECRET=your_client_secret
```

## Análise de email

Depois de ligado, o Campbooks analisa automaticamente a sua caixa de entrada. Pode:

- **Iniciar manualmente** uma análise a partir da página de Análises de Email
- **Agendar análises recorrentes** via tarefas recorrentes do Solid Queue
- **Filtrar por pasta** (Caixa de entrada, Enviados, etc.)

Cada análise obtém mensagens via a API do fornecedor e transfere os anexos. Consulte [Análise de Email](/docs/email/scanning) para mais detalhes.
