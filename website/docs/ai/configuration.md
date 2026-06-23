---
title: "AI Configuration"
description: "How to configure AI adapters in Campbooks"
sidebar_position: 1
---

Campbooks uses AI for document classification, email analysis, and chat. You can configure multiple AI providers and assign them to different services.

## AI services

Campbooks has six AI services, each configurable independently:

| Service | Purpose | Recommended model |
|---------|---------|-------------------|
| Document Analysis | Classify and analyze document attachments | Claude Sonnet (vision) |
| Email Classification | Categorize emails by type | Claude Haiku or DeepSeek |
| Email Analysis | Analyze email content for action items | Claude Sonnet |
| Email Chat | AI chat about emails | Claude Sonnet |
| Draft Reply | Generate email reply drafts | Claude Sonnet |
| Global Chat | General AI assistant (Scout) | Claude Sonnet |

## Adding an AI adapter

1. Go to **Settings → AI Configuration**
2. Click **Add Adapter**
3. Choose the provider type
4. Enter the API key and endpoint
5. Test the connection

## Supported providers

### Anthropic (Claude)

- **Model**: Claude Opus, Sonnet, or Haiku
- **API Key**: From [console.anthropic.com](https://console.anthropic.com/)
- **Endpoint**: `https://api.anthropic.com`

### OpenAI

- **Model**: GPT-4 Vision, GPT-4o, GPT-4o-mini
- **API Key**: From [platform.openai.com](https://platform.openai.com/)
- **Endpoint**: `https://api.openai.com`

### OpenAI-compatible

Any provider with an OpenAI-compatible API works. This includes:

- **DeepSeek** (`https://api.deepseek.com`)
- **OpenRouter** (`https://openrouter.ai`)
- **Groq** (`https://api.groq.com`)
- **Ollama** (local, `http://localhost:11434`)
- **LM Studio** (local, `http://localhost:1234`)

## Assigning adapters to services

Each AI service can use a different adapter:

1. Go to **Settings → AI Configuration**
2. Find the service you want to configure
3. Select the adapter from the dropdown
4. The service will immediately start using the new adapter

This lets you optimize for cost and capability — use a fast, cheap model for classification and a more capable model for analysis and chat.

## Vision support

Document analysis requires a **vision-capable** model. If you're using an adapter that doesn't support vision, document analysis will fail. Supported vision models:

- Claude Sonnet, Claude Opus
- GPT-4 Vision, GPT-4o
- Gemini 1.5 Flash/Pro (via OpenAI-compatible)

## Testing

After configuring an adapter, test it by sending a message in the Scout chat or by reprocessing a document. If there's an error, check:

- The API key is correct
- The endpoint URL is correct
- The model name matches what the provider expects
- The provider supports the feature you're using (e.g., vision for document analysis)
