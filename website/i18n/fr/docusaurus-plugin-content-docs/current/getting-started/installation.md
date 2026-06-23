---
title: "Installation"
description: "Comment installer et exécuter Campbooks sur votre propre serveur"
sidebar_position: 2
---

Installez Campbooks sur votre propre serveur. Vous aurez besoin de Ruby 3.3+, PostgreSQL 16+ et Node.js 18+.

<div class="callout callout-note">
  **Nouveau avec Rails ?** Campbooks est une application Rails standard. Si vous avez déjà déployé Rails, cela vous semblera familier. La plupart des étapes suivent les conventions Rails.
</div>

## Prérequis

- **Ruby** 3.3 ou version ultérieure
- **PostgreSQL** 16 ou version ultérieure
- **Node.js** 18 ou version ultérieure
- **Redis** (pour Action Cable, facultatif — utilise Solid Cable par défaut)
- **OpenSearch** (pour la recherche plein texte, facultatif — utilise PostgreSQL par défaut)

## Cloner le dépôt

```bash
git clone https://github.com/notacamp/campbooks.git
cd campbooks
```

## Installer les dépendances

```bash
bundle install
```

## Configurer la base de données

```bash
bin/rails db:create
bin/rails db:migrate
bin/rails db:seed
```

<div class="callout callout-info">
  **Utilisateurs de seed.** La commande seed crée deux comptes pour les tests :
  `admin@example.com` et `partner@example.com`, tous deux avec le mot de passe `changeme123`.
</div>

## Configurer les variables d'environnement

Copiez le fichier d'environnement exemple et renseignez vos valeurs :

```bash
cp .env.example .env
```

**Obligatoires :**

| Variable | Rôle |
|----------|------|
| `DATABASE_URL` | Chaîne de connexion PostgreSQL |
| `ACTIVE_RECORD_PRIMARY_KEY` | Clé primaire de chiffrement |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Clé de chiffrement déterministe |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Sel de dérivation de clé de chiffrement |

**Facultatives mais recommandées :**

| Variable | Rôle |
|----------|------|
| `ANTHROPIC_API_KEY` | Clé API Claude pour les fonctionnalités IA |
| `OPENAI_API_KEY` | Clé API OpenAI pour les embeddings |
| `ZOHO_CLIENT_ID` / `ZOHO_CLIENT_SECRET` | Identifiants OAuth Zoho Mail |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | Identifiants OAuth Google |

<div class="callout callout-warning">
  **Les clés de chiffrement sont obligatoires.** Sans `ACTIVE_RECORD_PRIMARY_KEY`, `ACTIVE_RECORD_DETERMINISTIC_KEY` et `ACTIVE_RECORD_KEY_DERIVATION_SALT`, l'application ne démarrera pas. Générez-les avec `bin/rails secret` et utilisez la valeur obtenue pour chaque clé.
</div>

Générer les clés de chiffrement :

```bash
bin/rails secret
```

## Démarrer l'application

```bash
bin/rails server               # Serveur web sur le port 3000
bin/rails solid_queue:start    # Worker de tâches en arrière-plan
```

Ou avec le Procfile :

```bash
bin/dev
```

Ouvrez `http://localhost:3000` et connectez-vous avec l'un des utilisateurs de seed.

## Docker

Un Dockerfile est fourni pour les déploiements en production :

```bash
docker build -t campbooks .
docker run -p 3000:3000 --env-file .env campbooks
```

<div class="callout callout-note">
  **Étape suivante.** Consultez le [guide de déploiement](/docs/deployment/overview) pour une configuration complète en production avec Nginx, SSL et systemd.
</div>
