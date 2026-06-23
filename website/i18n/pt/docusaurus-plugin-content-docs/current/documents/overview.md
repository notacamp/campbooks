---
title: "Visão Geral dos Documentos"
description: "Como funciona a gestão de documentos no Campbooks"
sidebar_position: 1
---

Os documentos são o núcleo do Campbooks. Cada anexo de email torna-se um documento que é classificado, analisado e acompanhado ao longo do fluxo de revisão.

## Ciclo de vida de um documento

1. **Recebido** — um anexo de email é recebido e um documento é criado
2. **Em processamento** — a IA analisa o documento para determinar o tipo e extrair dados
3. **Pronto para revisão** — o documento aparece no painel, pronto para ser revisto
4. **Aprovado / Rejeitado** — aprova ou rejeita o documento
5. **Exportado** — opcionalmente, envia o documento para o Google Drive, Zoho WorkDrive ou Notion

## Tipos de documento

O Campbooks consegue reconhecer estes tipos de documento (e pode adicionar tipos personalizados):

- Faturas
- Recibos
- Contratos
- Extratos
- Documentos fiscais
- Relatórios
- Formulários
- Correspondência

A classificação por IA utiliza o Claude Vision para analisar cada documento e sugerir o tipo adequado.

## Estados dos documentos

| Estado | Significado |
|--------|-------------|
| Pendente | Acabou de ser recebido, aguarda processamento |
| Em processamento | Análise por IA em curso |
| Necessita revisão | Pronto para revisão humana |
| Aprovado | Revisto e aprovado |
| Rejeitado | Revisto e rejeitado |
| Falhou | O processamento encontrou um erro |

## Pesquisa e filtros

Os documentos podem ser pesquisados por:

- Texto completo (via OpenSearch ou PostgreSQL)
- Tipo de documento
- Estado
- Intervalo de datas
- Conta de email de origem
- Etiquetas
