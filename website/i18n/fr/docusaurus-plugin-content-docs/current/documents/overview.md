---
title: "Vue d'ensemble des documents"
description: "Comment fonctionne la gestion documentaire dans Campbooks"
sidebar_position: 1
---

Les documents sont au cœur de Campbooks. Chaque pièce jointe reçue par e-mail devient un document classifié, analysé et suivi tout au long de votre flux de validation.

## Cycle de vie d'un document

1. **Ingéré** — une pièce jointe est reçue et un document est créé
2. **En traitement** — l'IA analyse le document pour en déterminer le type et en extraire les données
3. **Prêt à réviser** — le document apparaît dans votre tableau de bord, prêt à être examiné
4. **Approuvé / Rejeté** — vous approuvez ou rejetez le document
5. **Exporté** — en option, le document est transmis vers Google Drive, Zoho WorkDrive ou Notion

## Types de documents

Campbooks reconnaît les types de documents suivants (et vous pouvez en ajouter des personnalisés) :

- Factures
- Reçus
- Contrats
- Relevés
- Documents fiscaux
- Rapports
- Formulaires
- Correspondances

La classification par IA utilise Claude Vision pour analyser chaque document et suggérer le type approprié.

## Statuts des documents

| Statut | Signification |
|--------|---------------|
| En attente | Tout juste ingéré, en attente de traitement |
| En traitement | Analyse IA en cours |
| À réviser | Prêt pour examen humain |
| Approuvé | Examiné et approuvé |
| Rejeté | Examiné et rejeté |
| Échec | Une erreur est survenue lors du traitement |

## Recherche et filtrage

Les documents sont consultables par :

- Texte intégral (via OpenSearch ou PostgreSQL)
- Type de document
- Statut
- Plage de dates
- Compte e-mail source
- Étiquettes
