---
title: "Configuração de IA"
description: "Como configurar os adaptadores de IA no Campbooks"
sidebar_position: 1
---

O Campbooks utiliza IA para classificação de documentos, análise de emails e chat. Pode configurar múltiplos fornecedores de IA e atribuí-los a diferentes serviços.

## Serviços de IA

O Campbooks tem seis serviços de IA, cada um configurável de forma independente:

| Serviço | Finalidade | Modelo recomendado |
|---------|---------|-------------------|
| Análise de Documentos | Classificar e analisar anexos de documentos | Claude Sonnet (visão) |
| Classificação de Email | Categorizar emails por tipo | Claude Haiku ou DeepSeek |
| Análise de Email | Analisar o conteúdo de emails para itens de ação | Claude Sonnet |
| Chat de Email | Chat com IA sobre emails | Claude Sonnet |
| Rascunho de Resposta | Gerar rascunhos de resposta a emails | Claude Sonnet |
| Chat Global | Assistente de IA geral (Scout) | Claude Sonnet |

## Adicionar um adaptador de IA

1. Aceda a **Definições → Configuração de IA**
2. Clique em **Adicionar Adaptador**
3. Escolha o tipo de fornecedor
4. Introduza a chave de API e o endpoint
5. Teste a ligação

## Fornecedores suportados

### Anthropic (Claude)

- **Modelo**: Claude Opus, Sonnet ou Haiku
- **Chave de API**: Em [console.anthropic.com](https://console.anthropic.com/)
- **Endpoint**: `https://api.anthropic.com`

### OpenAI

- **Modelo**: GPT-4 Vision, GPT-4o, GPT-4o-mini
- **Chave de API**: Em [platform.openai.com](https://platform.openai.com/)
- **Endpoint**: `https://api.openai.com`

### Compatíveis com OpenAI

Qualquer fornecedor com uma API compatível com OpenAI funciona. Isto inclui:

- **DeepSeek** (`https://api.deepseek.com`)
- **OpenRouter** (`https://openrouter.ai`)
- **Groq** (`https://api.groq.com`)
- **Ollama** (local, `http://localhost:11434`)
- **LM Studio** (local, `http://localhost:1234`)

## Atribuir adaptadores a serviços

Cada serviço de IA pode utilizar um adaptador diferente:

1. Aceda a **Definições → Configuração de IA**
2. Localize o serviço que pretende configurar
3. Selecione o adaptador no menu pendente
4. O serviço começará imediatamente a utilizar o novo adaptador

Isto permite-lhe otimizar o custo e a capacidade — utilize um modelo rápido e económico para classificação e um modelo mais capaz para análise e chat.

## Suporte a visão

A análise de documentos requer um modelo com **capacidade de visão**. Se estiver a utilizar um adaptador que não suporte visão, a análise de documentos irá falhar. Modelos com suporte a visão:

- Claude Sonnet, Claude Opus
- GPT-4 Vision, GPT-4o
- Gemini 1.5 Flash/Pro (via compatível com OpenAI)

## Testes

Após configurar um adaptador, teste-o enviando uma mensagem no chat Scout ou reprocessando um documento. Se ocorrer um erro, verifique:

- Se a chave de API está correta
- Se o URL do endpoint está correto
- Se o nome do modelo corresponde ao que o fornecedor espera
- Se o fornecedor suporta a funcionalidade que está a utilizar (por exemplo, visão para análise de documentos)
