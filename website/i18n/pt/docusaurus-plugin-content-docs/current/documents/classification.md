---
title: "Classificação de Documentos"
description: "Como a IA classifica documentos no Campbooks"
sidebar_position: 2
---

O Campbooks utiliza IA para classificar documentos automaticamente, determinar o seu tipo e extrair informação relevante.

## Como funciona a classificação

1. Quando um documento é criado a partir de um anexo de email, entra na fila de processamento
2. O **Claude** (Anthropic) analisa o documento com visão computacional para determinar o seu tipo
3. A IA extrai dados estruturados — nomes de fornecedores, montantes, datas, números de fatura, etc.
4. O documento é etiquetado e categorizado com base na análise

## Configurar a IA para classificação

A classificação requer um adaptador de IA configurado para análise de documentos:

1. Vá a **Definições → Configuração de IA**
2. Adicione um adaptador de IA (Anthropic, OpenAI ou compatível)
3. Atribua o adaptador ao serviço de **Análise de Documentos**

O serviço de análise de documentos utiliza tipicamente um modelo com capacidade de visão, como o Claude Sonnet ou o GPT-4 Vision.

## Tipos de documento personalizados

Pode definir tipos de documento personalizados:

1. Vá a **Definições → Tipos de Documento**
2. Adicione um novo tipo com um nome e descrição
3. A IA passará a reconhecer este tipo juntamente com os tipos predefinidos

Durante a integração inicial, o Campbooks pode sugerir tipos de documento com base na descrição da sua organização.

## Rever classificações

As classificações da IA não são definitivas. Pode:

- **Aprovar** a classificação se estiver correta
- **Alterar o tipo** se a IA errou
- **Reprocessar** o documento para repetir a classificação

Cada correção ajuda a perceber em que áreas a IA é mais eficaz (e contribui para orientar futuras escolhas de modelos).

## Fornecedores de IA

O Campbooks suporta vários fornecedores de IA para análise de documentos:

- **Claude (Anthropic)** — recomendado, excelente na compreensão de documentos
- **GPT-4 Vision (OpenAI)** — alternativa robusta
- **DeepSeek** — opção económica via API compatível com OpenAI
- **Fornecedores compatíveis com OpenAI** — qualquer fornecedor com uma API no formato OpenAI

Cada adaptador é configurado com um endpoint, chave de API e versão de modelo. Pode utilizar diferentes fornecedores para diferentes serviços — por exemplo, Claude para análise de documentos e DeepSeek para classificação de emails.
