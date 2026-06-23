---
title: "Analyse des e-mails"
description: "Comment Campbooks analyse et traite les messages e-mail"
sidebar_position: 2
---

Campbooks analyse vos comptes e-mail connectés et traite les messages ainsi que leurs pièces jointes.

## Comment fonctionne l'analyse

1. **Tâche d'analyse** : une tâche d'arrière-plan récupère les messages depuis l'API de votre fournisseur e-mail
2. **Déduplication** : les messages sont identifiés par leur identifiant de message fournisseur afin d'éviter les doublons
3. **Traitement** : chaque nouveau message est traité pour télécharger ses pièces jointes
4. **Création de documents** : les pièces jointes deviennent des documents classifiés par IA

## Analyse manuelle

Rendez-vous dans **Analyses e-mail** et cliquez sur « Nouvelle analyse ». Sélectionnez le compte e-mail et le dossier à analyser.

## Analyse automatique

Campbooks exécute des analyses récurrentes selon un calendrier configurable par compte e-mail. L'intervalle par défaut est de 5 minutes.

## Statut de l'analyse

| Statut | Signification |
|--------|---------------|
| En attente | Analyse en file d'attente, en attente d'exécution |
| En cours | Analyse en cours d'exécution |
| Terminée | Analyse achevée avec succès |
| Échec | Une erreur est survenue lors de l'analyse |

## Traitement des messages

Chaque message analysé suit un pipeline de traitement :

1. **Téléchargement des pièces jointes** — les fichiers sont stockés via Active Storage
2. **Création de documents** — un enregistrement Document est créé pour chaque pièce jointe
3. **Classification IA** — le type de document est déterminé par l'IA
4. **Indexation** — le document est indexé pour la recherche en texte intégral

## Consulter les messages

Les messages analysés apparaissent dans la section **Messages e-mail**. Vous pouvez :

- Consulter le message complet, y compris ses pièces jointes
- Répondre à l'expéditeur
- Ajouter des étiquettes et des libellés
- Voir les éléments d'action et les suggestions générés par l'IA
