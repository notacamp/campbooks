---
title: "Clasificación de documentos"
description: "Cómo la IA clasifica los documentos en Campbooks"
sidebar_position: 2
---

Campbooks utiliza IA para clasificar documentos automáticamente, determinar su tipo y extraer información relevante.

## Cómo funciona la clasificación

1. Cuando se crea un documento a partir de un adjunto de correo, entra en la cola de procesamiento
2. **Claude** (Anthropic) analiza el documento mediante visión por computadora para determinar su tipo
3. La IA extrae datos estructurados: nombres de proveedores, importes, fechas, números de factura, etc.
4. El documento se etiqueta y categoriza en función del análisis

## Configurar la IA para la clasificación

La clasificación requiere un adaptador de IA configurado para el análisis de documentos:

1. Ve a **Ajustes → Configuración de IA**
2. Añade un adaptador de IA (Anthropic, OpenAI u otro compatible)
3. Asigna el adaptador al servicio de **Análisis de documentos**

El servicio de análisis de documentos suele utilizar un modelo con capacidades de visión, como Claude Sonnet o GPT-4 Vision.

## Tipos de documento personalizados

Puedes definir tipos de documento personalizados:

1. Ve a **Ajustes → Tipos de documento**
2. Añade un nuevo tipo con nombre y descripción
3. La IA empezará a reconocer este tipo junto a los predefinidos

Durante la incorporación, Campbooks puede sugerir tipos de documento basándose en la descripción de tu organización.

## Revisar las clasificaciones

Las clasificaciones de la IA no son definitivas. Puedes:

- **Aprobar** la clasificación si es correcta
- **Cambiar el tipo** si la IA se ha equivocado
- **Reprocesar** el documento para ejecutar la clasificación de nuevo

Cada corrección te ayuda a entender en qué es buena la IA (y sirve de guía para futuras selecciones de modelo).

## Proveedores de IA

Campbooks es compatible con varios proveedores de IA para el análisis de documentos:

- **Claude (Anthropic)** — recomendado, excelente comprensión de documentos
- **GPT-4 Vision (OpenAI)** — alternativa sólida
- **DeepSeek** — opción económica mediante API compatible con OpenAI
- **Proveedores compatibles con OpenAI** — cualquier proveedor con una API que coincida con el formato de OpenAI

Cada adaptador se configura con un endpoint, una clave de API y una versión de modelo. Puedes utilizar distintos proveedores para distintos servicios; por ejemplo, Claude para el análisis de documentos y DeepSeek para la clasificación de correos.
