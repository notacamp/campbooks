---
title: "Classification des documents"
description: "Comment l'IA classe les documents dans Campbooks"
sidebar_position: 2
---

Campbooks utilise l'IA pour classer automatiquement les documents, déterminer leur type et extraire les informations pertinentes.

## Comment fonctionne la classification

1. Lorsqu'un document est créé à partir d'une pièce jointe, il entre dans la file de traitement
2. **Claude** (Anthropic) analyse le document par vision par ordinateur pour en déterminer le type
3. L'IA extrait des données structurées — noms de fournisseurs, montants, dates, numéros de facture, etc.
4. Le document est étiqueté et catégorisé en fonction de l'analyse

## Configurer l'IA pour la classification

La classification nécessite un adaptateur IA configuré pour l'analyse documentaire :

1. Rendez-vous dans **Paramètres → Configuration IA**
2. Ajoutez un adaptateur IA (Anthropic, OpenAI ou compatible)
3. Affectez l'adaptateur au service **Analyse de documents**

Le service d'analyse de documents utilise généralement un modèle capable de vision, tel que Claude Sonnet ou GPT-4 Vision.

## Types de documents personnalisés

Vous pouvez définir des types de documents personnalisés :

1. Rendez-vous dans **Paramètres → Types de documents**
2. Ajoutez un nouveau type avec un nom et une description
3. L'IA commencera à reconnaître ce type aux côtés des types intégrés

Lors de l'intégration initiale, Campbooks peut suggérer des types de documents adaptés à la description de votre organisation.

## Réviser les classifications

Les classifications de l'IA ne sont pas définitives. Vous pouvez :

- **Approuver** la classification si elle est correcte
- **Modifier le type** si l'IA s'est trompée
- **Retraiter** le document pour relancer la classification

Chaque correction vous aide à comprendre les points forts de l'IA (et à éclairer le choix futur des modèles).

## Fournisseurs IA

Campbooks prend en charge plusieurs fournisseurs IA pour l'analyse documentaire :

- **Claude (Anthropic)** — recommandé, excellente compréhension des documents
- **GPT-4 Vision (OpenAI)** — alternative solide
- **DeepSeek** — option économique via une API compatible OpenAI
- **Fournisseurs compatibles OpenAI** — tout fournisseur disposant d'une API conforme au format OpenAI

Chaque adaptateur est configuré avec un point de terminaison, une clé API et une version de modèle. Vous pouvez utiliser des fournisseurs différents selon les services — par exemple, Claude pour l'analyse documentaire et DeepSeek pour la classification des e-mails.
