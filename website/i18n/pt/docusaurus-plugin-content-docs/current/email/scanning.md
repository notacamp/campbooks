---
title: "Análise de Email"
description: "Como o Campbooks analisa e processa mensagens de email"
sidebar_position: 2
---

O Campbooks analisa as suas contas de email ligadas e processa mensagens e anexos.

## Como funciona a análise

1. **Tarefa de análise**: uma tarefa em segundo plano obtém mensagens da API do seu fornecedor de email
2. **Deduplicação**: as mensagens são identificadas pelo ID de mensagem do fornecedor para evitar duplicados
3. **Processamento**: cada nova mensagem é processada para transferir os anexos
4. **Criação de documentos**: os anexos tornam-se Documentos com classificação por IA

## Análise manual

Vá a **Análises de Email** e clique em "Nova Análise". Selecione a conta de email e a pasta a analisar.

## Análise automática

O Campbooks executa análises recorrentes segundo um agendamento (configurável por conta de email). O intervalo predefinido é de 5 em 5 minutos.

## Estado da análise

| Estado | Significado |
|--------|-------------|
| Pendente | Análise em fila, aguarda execução |
| Em execução | Análise em curso |
| Concluída | Análise terminada com sucesso |
| Falhou | A análise encontrou um erro |

## Processamento de mensagens

Cada mensagem analisada passa por uma cadeia de processamento:

1. **Transferência de anexos** — os ficheiros são armazenados via Active Storage
2. **Criação de documentos** — é criado um registo de Documento para cada anexo
3. **Classificação por IA** — o tipo de documento é determinado pela IA
4. **Indexação** — o documento é indexado para pesquisa de texto completo

## Visualizar mensagens

As mensagens analisadas aparecem na secção **Mensagens de Email**. Pode:

- Ver a mensagem completa, incluindo os anexos
- Responder ao remetente
- Adicionar etiquetas
- Consultar itens de ação e sugestões gerados pela IA
