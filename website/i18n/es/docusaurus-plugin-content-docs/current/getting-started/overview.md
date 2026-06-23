---
title: "Descripción general"
description: "Qué es Campbooks y cómo funciona"
sidebar_position: 1
---

Campbooks es un cliente de correo electrónico nativo de IA con codigo fuente disponible para profesionales y pequeñas empresas. Lee tus correos y documentos, usa IA para archivar y destacar lo que importa, y te ofrece un flujo de trabajo claro de revisión y aprobación, reinventado para que no se parezca en nada al correo al que estás acostumbrado.

## Qué hace Campbooks

- **Ingesta correos electrónicos** de Zoho Mail, Google Workspace o Microsoft 365 mediante OAuth
- **Clasifica documentos** con IA — facturas, contratos, recibos y más
- **Prioriza las acciones pendientes** — sabe qué necesita tu atención en este momento
- **Gestiona aprobaciones** — revisa, aprueba o rechaza documentos
- **Exporta a tus herramientas** — Google Drive, Zoho WorkDrive, Notion

## Cómo funciona

1. **Conecta una cuenta de correo** mediante OAuth. Campbooks analiza tu bandeja de entrada en busca de correos con archivos adjuntos.
2. **La IA clasifica** cada adjunto — reconociendo tipos de documentos como facturas, contratos, recibos y más.
3. **Los documentos aparecen en tu panel** con estados y elementos de acción. Puedes revisarlos, aprobarlos o exportarlos.
4. **La integración con el correo** te permite responder, etiquetar y organizar correos directamente desde Campbooks.

## Arquitectura

Campbooks es una aplicación Ruby on Rails con:

- **PostgreSQL** como base de datos
- **Solid Queue** para el procesamiento de tareas en segundo plano
- **Tailwind CSS** para la interfaz
- **Hotwire** para las funciones interactivas
- **Claude (Anthropic)** para el análisis y la clasificación de documentos con IA

## Próximos pasos

- [Instala Campbooks](/docs/getting-started/installation) en tu servidor
- [Conecta una cuenta de correo](/docs/email/connecting-accounts)
- [Configura los servicios de IA](/docs/ai/configuration)
