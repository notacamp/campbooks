---
title: "Escaneo de correo electrónico"
description: "Cómo Campbooks escanea y procesa los mensajes de correo electrónico"
sidebar_position: 2
---

Campbooks escanea tus cuentas de correo conectadas y procesa los mensajes y sus adjuntos.

## Cómo funciona el escaneo

1. **Trabajo de escaneo**: un trabajo en segundo plano obtiene los mensajes de la API de tu proveedor de correo
2. **Deduplicación**: los mensajes se identifican por el ID de mensaje del proveedor para evitar duplicados
3. **Procesamiento**: cada mensaje nuevo se procesa para descargar sus adjuntos
4. **Creación de documentos**: los adjuntos se convierten en documentos con clasificación de IA

## Escaneo manual

Ve a **Escaneos de correo** y haz clic en "Nuevo escaneo". Selecciona la cuenta de correo y la carpeta que quieres escanear.

## Escaneo automático

Campbooks ejecuta escaneos recurrentes según una programación (configurable por cuenta de correo). El intervalo predeterminado es cada 5 minutos.

## Estado del escaneo

| Estado | Significado |
|--------|-------------|
| Pendiente | Escaneo en cola, esperando ejecución |
| En curso | Escaneo en progreso |
| Completado | Escaneo finalizado correctamente |
| Error | El escaneo encontró un error |

## Procesamiento de mensajes

Cada mensaje escaneado pasa por una cadena de procesamiento:

1. **Descarga de adjuntos** — los archivos se almacenan mediante Active Storage
2. **Creación de documentos** — se crea un registro Document por cada adjunto
3. **Clasificación con IA** — la IA determina el tipo de documento
4. **Indexación** — el documento se indexa para búsqueda de texto completo

## Ver mensajes

Los mensajes escaneados aparecen en la sección **Mensajes de correo**. Puedes:

- Ver el mensaje completo, incluidos los adjuntos
- Responder al remitente
- Añadir etiquetas y marcadores
- Ver los elementos de acción y sugerencias generados por la IA
