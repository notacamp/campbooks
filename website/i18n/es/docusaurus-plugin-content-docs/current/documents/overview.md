---
title: "Visión general de documentos"
description: "Cómo funciona la gestión de documentos en Campbooks"
sidebar_position: 1
---

Los documentos son el núcleo de Campbooks. Cada adjunto de correo electrónico se convierte en un documento que se clasifica, analiza y sigue a lo largo de tu flujo de revisión.

## Ciclo de vida de un documento

1. **Ingestado** — se recibe un adjunto de correo y se crea el documento
2. **Procesando** — la IA analiza el documento para determinar su tipo y extraer datos
3. **Listo para revisión** — el documento aparece en tu panel, listo para revisar
4. **Aprobado / Rechazado** — apruebas o rechazas el documento
5. **Exportado** — opcionalmente, envías el documento a Google Drive, Zoho WorkDrive o Notion

## Tipos de documento

Campbooks puede reconocer los siguientes tipos de documento (y puedes añadir tipos personalizados):

- Facturas
- Recibos
- Contratos
- Extractos
- Documentos fiscales
- Informes
- Formularios
- Correspondencia

La clasificación con IA utiliza Claude Vision para analizar cada documento y sugerir el tipo adecuado.

## Estados de documento

| Estado | Significado |
|--------|-------------|
| Pendiente | Recién ingestado, esperando procesamiento |
| Procesando | Análisis de IA en curso |
| Pendiente de revisión | Listo para revisión humana |
| Aprobado | Revisado y aprobado |
| Rechazado | Revisado y rechazado |
| Error | El procesamiento encontró un error |

## Búsqueda y filtrado

Los documentos se pueden buscar por:

- Texto completo (mediante OpenSearch o PostgreSQL)
- Tipo de documento
- Estado
- Rango de fechas
- Cuenta de correo de origen
- Etiquetas
