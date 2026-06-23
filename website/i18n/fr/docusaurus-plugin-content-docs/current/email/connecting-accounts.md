---
title: "Connecter des comptes e-mail"
description: "Comment connecter des comptes e-mail à Campbooks via OAuth"
sidebar_position: 1
---

Campbooks se connecte à votre fournisseur e-mail via OAuth. Il prend en charge Zoho Mail, Google Workspace et Microsoft 365.

## Fournisseurs pris en charge

| Fournisseur | Protocole | Configuration |
|-------------|-----------|---------------|
| Zoho Mail | API REST | Zoho Developer Console |
| Google Workspace | API Gmail | Google Cloud Console |
| Microsoft 365 | Microsoft Graph | Inscription d'application Azure |

## Zoho Mail

### 1. Créer un client API Zoho

1. Rendez-vous dans la [Zoho Developer Console](https://api-console.zoho.com/)
2. Créez une nouvelle **Application basée sur serveur**
3. Ajoutez l'URI de redirection : `https://your-domain.com/oauth/zoho/callback`
4. Notez votre **Client ID** et votre **Client Secret**

### 2. Configurer les variables d'environnement

```bash
ZOHO_CLIENT_ID=your_client_id
ZOHO_CLIENT_SECRET=your_client_secret
```

### 3. Connecter dans Campbooks

1. Rendez-vous dans Paramètres → Comptes e-mail
2. Cliquez sur « Ajouter un compte e-mail »
3. Sélectionnez Zoho Mail
4. Vous serez redirigé vers Zoho pour autoriser la connexion
5. Après autorisation, Campbooks stocke un jeton de rafraîchissement chiffré

## Google Workspace

### 1. Créer un projet Google Cloud

1. Rendez-vous dans la [Google Cloud Console](https://console.cloud.google.com/)
2. Créez un nouveau projet
3. Activez l'**API Gmail**
4. Créez des identifiants OAuth 2.0 (Application Web)
5. Ajoutez l'URI de redirection : `https://your-domain.com/oauth/google/callback`

### 2. Configurer les variables d'environnement

```bash
GOOGLE_CLIENT_ID=your_client_id
GOOGLE_CLIENT_SECRET=your_client_secret
```

### 3. Connecter dans Campbooks

1. Rendez-vous dans Paramètres → Comptes e-mail
2. Cliquez sur « Ajouter un compte e-mail »
3. Sélectionnez Google Workspace
4. Autorisez via l'écran de consentement OAuth de Google

## Microsoft 365

### 1. Inscrire une application Azure

1. Rendez-vous dans le [Portail Azure](https://portal.azure.com/) → Inscriptions d'applications
2. Inscrivez une nouvelle application
3. Ajoutez l'URI de redirection : `https://your-domain.com/oauth/microsoft/callback`
4. Générez un secret client

### 2. Configurer les variables d'environnement

```bash
MICROSOFT_CLIENT_ID=your_client_id
MICROSOFT_CLIENT_SECRET=your_client_secret
```

## Analyse des e-mails

Une fois connecté, Campbooks analyse automatiquement votre boîte de réception. Vous pouvez :

- **Déclencher manuellement** une analyse depuis la page Analyses e-mail
- **Planifier des analyses récurrentes** via les tâches récurrentes de Solid Queue
- **Filtrer par dossier** (Boîte de réception, Envoyés, etc.)

Chaque analyse récupère les messages via l'API du fournisseur et télécharge les pièces jointes. Consultez [Analyse des e-mails](/docs/email/scanning) pour plus de détails.
