---
title: "Configuration de l'IA"
description: "Comment configurer les adaptateurs IA dans Campbooks"
sidebar_position: 1
---

Campbooks utilise l'IA pour la classification des documents, l'analyse des e-mails et le chat. Vous pouvez configurer plusieurs fournisseurs IA et les affecter à différents services.

## Services IA

Campbooks dispose de six services IA, chacun configurable indépendamment :

| Service | Rôle | Modèle recommandé |
|---------|------|-------------------|
| Analyse de documents | Classer et analyser les pièces jointes | Claude Sonnet (vision) |
| Classification d'e-mails | Catégoriser les e-mails par type | Claude Haiku ou DeepSeek |
| Analyse d'e-mails | Analyser le contenu des e-mails pour identifier les actions requises | Claude Sonnet |
| Chat e-mail | Chat IA sur les e-mails | Claude Sonnet |
| Rédaction de réponse | Générer des brouillons de réponse aux e-mails | Claude Sonnet |
| Chat global | Assistant IA général (Scout) | Claude Sonnet |

## Ajouter un adaptateur IA

1. Allez dans **Paramètres → Configuration IA**
2. Cliquez sur **Ajouter un adaptateur**
3. Choisissez le type de fournisseur
4. Saisissez la clé API et le point de terminaison
5. Testez la connexion

## Fournisseurs pris en charge

### Anthropic (Claude)

- **Modèle** : Claude Opus, Sonnet ou Haiku
- **Clé API** : Depuis [console.anthropic.com](https://console.anthropic.com/)
- **Point de terminaison** : `https://api.anthropic.com`

### OpenAI

- **Modèle** : GPT-4 Vision, GPT-4o, GPT-4o-mini
- **Clé API** : Depuis [platform.openai.com](https://platform.openai.com/)
- **Point de terminaison** : `https://api.openai.com`

### Compatible OpenAI

Tout fournisseur disposant d'une API compatible OpenAI fonctionne. Cela inclut :

- **DeepSeek** (`https://api.deepseek.com`)
- **OpenRouter** (`https://openrouter.ai`)
- **Groq** (`https://api.groq.com`)
- **Ollama** (local, `http://localhost:11434`)
- **LM Studio** (local, `http://localhost:1234`)

## Affecter des adaptateurs aux services

Chaque service IA peut utiliser un adaptateur différent :

1. Allez dans **Paramètres → Configuration IA**
2. Trouvez le service que vous souhaitez configurer
3. Sélectionnez l'adaptateur dans la liste déroulante
4. Le service commencera immédiatement à utiliser le nouvel adaptateur

Cela vous permet d'optimiser le rapport coût/performance — utilisez un modèle rapide et économique pour la classification, et un modèle plus capable pour l'analyse et le chat.

## Prise en charge de la vision

L'analyse de documents nécessite un modèle **compatible vision**. Si vous utilisez un adaptateur qui ne prend pas en charge la vision, l'analyse de documents échouera. Modèles vision pris en charge :

- Claude Sonnet, Claude Opus
- GPT-4 Vision, GPT-4o
- Gemini 1.5 Flash/Pro (via compatible OpenAI)

## Tests

Après avoir configuré un adaptateur, testez-le en envoyant un message dans le chat Scout ou en retraitant un document. En cas d'erreur, vérifiez :

- Que la clé API est correcte
- Que l'URL du point de terminaison est correcte
- Que le nom du modèle correspond à ce qu'attend le fournisseur
- Que le fournisseur prend en charge la fonctionnalité utilisée (par ex., la vision pour l'analyse de documents)
