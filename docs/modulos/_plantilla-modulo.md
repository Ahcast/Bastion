# Módulo: [nombre]

## Propósito y límite

Qué dominio cubre y qué responsabilidades conserva. Separar requisitos confirmados de propuestas nuevas.

## Pantallas y flujos actuales

Enumerar rutas, estados, pasos y alternativas con referencia a la sección del informe fuente.

## Campos

| Pantalla | Etiqueta | Tipo | Campo/API | Requerido | Validación/opciones | Efecto |
|---|---|---|---|---|---|---|

## Botones y acciones

| Pantalla | Botón/acción | Permiso | Efecto | Confirmación/mensaje | API/caso de uso |
|---|---|---|---|---|---|

## Reglas de negocio

Especificar cálculos, estados, transiciones, redondeos e invariantes.

## Settings (se presentan bajo Ajustes)

| Grupo | Campo | Tipo/defecto | Secreto | Validación | Permiso |
|---|---|---|---|---|---|

Estado: `Confirmado`, `Propuesto` o `Pendiente de decisión`. No marcar un setting propuesto como comportamiento heredado.

## Datos, API, permisos y MCP

Listar entidades propias, operaciones, permisos y herramientas MCP con riesgo/confirmación. Enlazar a contratos generales.

## Release y puesta en servicio

- Versión del módulo y rango de versiones del core compatibles.
- Migraciones propias, orden de aplicación y comprobación de versión.
- Configuración/flag para habilitarlo por ambiente.
- Pasos de validación en servidor y criterio para habilitarlo en producción.
- Procedimiento de deshabilitación/rollback que preserve sus datos.

## Criterios de aceptación y pendientes

Definir escenarios comprobables y preguntas no resueltas.
