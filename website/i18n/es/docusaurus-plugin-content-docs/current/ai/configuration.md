---
title: "Configuración de IA"
description: "Cómo configurar los adaptadores de IA en Campbooks"
sidebar_position: 1
---

Campbooks utiliza IA para la clasificación de documentos, el análisis de correos electrónicos y el chat. Puedes configurar varios proveedores de IA y asignarlos a diferentes servicios.

## Servicios de IA

Campbooks dispone de seis servicios de IA, cada uno configurable de forma independiente:

| Servicio | Propósito | Modelo recomendado |
|---------|---------|-------------------|
| Análisis de documentos | Clasificar y analizar documentos adjuntos | Claude Sonnet (visión) |
| Clasificación de correos | Categorizar correos por tipo | Claude Haiku o DeepSeek |
| Análisis de correos | Analizar el contenido del correo para detectar acciones | Claude Sonnet |
| Chat de correo | Chat de IA sobre correos electrónicos | Claude Sonnet |
| Borrador de respuesta | Generar borradores de respuesta a correos | Claude Sonnet |
| Chat global | Asistente de IA general (Scout) | Claude Sonnet |

## Añadir un adaptador de IA

1. Ve a **Configuración → Configuración de IA**
2. Haz clic en **Añadir adaptador**
3. Elige el tipo de proveedor
4. Introduce la clave de API y el endpoint
5. Prueba la conexión

## Proveedores compatibles

### Anthropic (Claude)

- **Modelo**: Claude Opus, Sonnet o Haiku
- **Clave de API**: Desde [console.anthropic.com](https://console.anthropic.com/)
- **Endpoint**: `https://api.anthropic.com`

### OpenAI

- **Modelo**: GPT-4 Vision, GPT-4o, GPT-4o-mini
- **Clave de API**: Desde [platform.openai.com](https://platform.openai.com/)
- **Endpoint**: `https://api.openai.com`

### Compatible con OpenAI

Cualquier proveedor con una API compatible con OpenAI funciona. Esto incluye:

- **DeepSeek** (`https://api.deepseek.com`)
- **OpenRouter** (`https://openrouter.ai`)
- **Groq** (`https://api.groq.com`)
- **Ollama** (local, `http://localhost:11434`)
- **LM Studio** (local, `http://localhost:1234`)

## Asignar adaptadores a servicios

Cada servicio de IA puede usar un adaptador diferente:

1. Ve a **Configuración → Configuración de IA**
2. Localiza el servicio que deseas configurar
3. Selecciona el adaptador en el desplegable
4. El servicio comenzará a usar el nuevo adaptador de inmediato

Esto te permite optimizar entre coste y capacidad — usa un modelo rápido y económico para la clasificación y un modelo más potente para el análisis y el chat.

## Compatibilidad con visión

El análisis de documentos requiere un modelo con **capacidad de visión**. Si usas un adaptador que no admite visión, el análisis de documentos fallará. Modelos de visión compatibles:

- Claude Sonnet, Claude Opus
- GPT-4 Vision, GPT-4o
- Gemini 1.5 Flash/Pro (mediante API compatible con OpenAI)

## Pruebas

Tras configurar un adaptador, pruébalo enviando un mensaje en el chat de Scout o reprocesando un documento. Si hay un error, comprueba:

- Que la clave de API sea correcta
- Que la URL del endpoint sea correcta
- Que el nombre del modelo coincida con lo que espera el proveedor
- Que el proveedor admita la función que estás usando (p. ej., visión para el análisis de documentos)
