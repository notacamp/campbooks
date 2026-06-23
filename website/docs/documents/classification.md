---
title: "Document Classification"
description: "How AI classifies documents in Campbooks"
sidebar_position: 2
---

Campbooks uses AI to automatically classify documents, determine their type, and extract relevant information.

## How classification works

1. When a document is created from an email attachment, it enters the processing queue
2. **Claude** (Anthropic) analyzes the document using computer vision to determine its type
3. The AI extracts structured data — vendor names, amounts, dates, invoice numbers, etc.
4. The document is tagged and categorized based on the analysis

## Configuring AI for classification

Classification requires an AI adapter configured for document analysis:

1. Go to **Settings → AI Configuration**
2. Add an AI adapter (Anthropic, OpenAI, or compatible)
3. Assign the adapter to the **Document Analysis** service

The document analysis service typically uses a vision-capable model like Claude Sonnet or GPT-4 Vision.

## Custom document types

You can define custom document types:

1. Go to **Settings → Document Types**
2. Add a new type with a name and description
3. The AI will start recognizing this type alongside the built-in ones

During onboarding, Campbooks can suggest document types based on your organization's description.

## Reviewing classifications

AI classifications aren't final. You can:

- **Approve** the classification if it's correct
- **Change the type** if the AI got it wrong
- **Reprocess** the document to re-run classification

Each correction helps you understand what the AI is good at (and helps inform future model selection).

## AI providers

Campbooks supports multiple AI providers for document analysis:

- **Claude (Anthropic)** — recommended, excellent at document understanding
- **GPT-4 Vision (OpenAI)** — strong alternative
- **DeepSeek** — cost-effective option via OpenAI-compatible API
- **OpenAI-compatible providers** — any provider with an API matching OpenAI's format

Each adapter is configured with an endpoint, API key, and model version. You can use different providers for different services — for example, Claude for document analysis and DeepSeek for email classification.
