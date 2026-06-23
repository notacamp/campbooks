---
title: "Vue d'ensemble"
description: "Ce qu'est Campbooks et comment il fonctionne"
sidebar_position: 1
---

Campbooks est un client de messagerie a code source disponible, concu autour de l'IA, pour les professionnels et les petites entreprises. Il lit vos e-mails et documents, utilise l'IA pour classer et mettre en avant ce qui compte, et vous offre un flux de révision et d'approbation clair — repensé pour qu'il ne ressemble en rien à la messagerie que vous connaissez.

## Ce que fait Campbooks

- **Ingère les e-mails** de Zoho Mail, Google Workspace ou Microsoft 365 via OAuth
- **Classe les documents** grâce à l'IA — factures, contrats, reçus, et bien plus
- **Priorise les actions requises** — sait ce qui nécessite votre attention immédiate
- **Gère les approbations** — révisez, approuvez ou rejetez des documents
- **Exporte vers vos outils** — Google Drive, Zoho WorkDrive, Notion

## Comment ça fonctionne

1. **Connectez un compte e-mail** via OAuth. Campbooks analyse votre boîte de réception à la recherche d'e-mails avec des pièces jointes.
2. **L'IA classe** chaque pièce jointe — en reconnaissant les types de documents comme les factures, contrats, reçus, et plus encore.
3. **Les documents apparaissent dans votre tableau de bord** avec leurs statuts et les actions requises. Vous pouvez les réviser, les approuver ou les exporter.
4. **L'intégration e-mail** vous permet de répondre, d'étiqueter et d'organiser les e-mails directement depuis Campbooks.

## Architecture

Campbooks est une application Ruby on Rails comprenant :

- **PostgreSQL** pour la base de données
- **Solid Queue** pour le traitement des tâches en arrière-plan
- **Tailwind CSS** pour l'interface
- **Hotwire** pour les fonctionnalités interactives
- **Claude (Anthropic)** pour l'analyse et la classification des documents par IA

## Prochaines étapes

- [Installer Campbooks](/docs/getting-started/installation) sur votre serveur
- [Connecter un compte e-mail](/docs/email/connecting-accounts)
- [Configurer les services IA](/docs/ai/configuration)
