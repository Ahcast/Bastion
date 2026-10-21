# Dirección visual del panel

## Fuente revisada

El usuario indicó que le gustan el diseño y los colores del panel de referencia. Se revisó como texto estático la exportación HTML local `C:\Users\Marco\Downloads\Bastion · Panel (1).html`. No se ejecutó el HTML ni su JavaScript.

La hoja CSS declara el concepto **“Bastion · Concreto”**: una interfaz mate y sobria, con masas de carbón para texto y acciones, reglas oscuras para estructurar y óxido para alertas. Las superficies son cuadradas y planas. La exportación define tokens con valores precisos; se toman como dirección visual confirmada por el usuario, pero deben validarse en la implementación por contraste y legibilidad.

## Decisiones visuales derivadas

- Compartir un solo sistema visual entre el core y todos los módulos. Los módulos consumen los tokens centrales; no definen paletas propias.
- Mantener una interfaz de escritorio densa y clara para operaciones, tablas y formularios, con adaptación a pantallas pequeñas.
- Usar el color óxido solo para alertas, errores o acciones destructivas. El verde comunica estados positivos.
- Usar reglas, líneas y espacios para separar contenido, no tarjetas con sombras decorativas.
- Conservar áreas de foco visibles y estados diferenciados de hover, activo, deshabilitado y error.
- Esta guía define apariencia. Para campos, acciones, permisos y flujo funcional, prevalece `01-aplicacion-actual-y-paridad.md` y las decisiones de producto confirmadas.

## Paleta extraída del CSS

| Token | Valor | Uso observado |
|---|---|---|
| `canvas` | `#ECECEC` | Fondo general y fondo del drawer. |
| `panel` | `#F8FFFF` | Superficie clara de áreas de trabajo y tarjetas planas. |
| `panel-raised` | `#D2D6D6` | Campos y superficies elevadas. |
| `line` | `#9AA2A3` | Separadores suaves y líneas de datos. |
| `line-strong` | `#404649` | Separadores fuertes. |
| `ink` | `#25292D` | Texto principal, navegación oscura, reglas y acciones primarias. |
| `ink-muted` | `#2F3438` | Texto secundario. |
| `ink-subtle` | `#3C4246` | Etiquetas y metadatos secundarios. |
| `accent` | `#25292D` | Acento principal. En la referencia coincide con `ink`. |
| `accent-strong` | `#3A3F44` | Acento oscuro reforzado. |
| `on-accent` | `#EEF0F0` | Texto sobre superficies oscuras. |
| `accent-dim` | `rgba(37, 41, 45, 0.07)` | Fondo tenue de acento. |
| `signal` | `#852A12` | Alertas, error y acción destructiva. |
| `signal-dim` | `rgba(156, 50, 22, 0.12)` | Fondo tenue de alerta, tal como está definido en el CSS. |
| `ok` | `#174D2E` | Indicadores positivos. |
| `chart-1` | `#34434B` | Serie de gráfico 1. |
| `chart-2` | `#4D565A` | Serie de gráfico 2. |
| `chart-3` | `#7F888B` | Serie de gráfico 3. |
| `chart-4` | `#A3ABAC` | Serie de gráfico 4 y segmentos inactivos. |

El CSS define una sombra flotante `0 12px 32px rgba(37, 41, 45, 0.18)`, aplicada al drawer de detalle. El resto del sistema se mantiene plano según el comentario de diseño. Confirmar contraste de cada combinación antes de publicar, en especial texto secundario sobre `panel-raised` y colores de gráficas.

## Tipografía y escala

- Texto general: **IBM Plex Sans**, con fuentes del sistema como fallback.
- Datos, códigos y etiquetas técnicas: **IBM Plex Mono**, con fuentes monoespaciadas del sistema como fallback.
- Lectura principal: 16/24 px, peso regular.
- Texto secundario: 14/20 px.
- Etiqueta: Mono 12/16 px, peso medio, espaciado de letra de 0.05 em.
- Título principal: 32/40 px, peso medio.
- Título de sección: 18/24 px, peso semibold.
- KPI: 34/40 px; cifra destacada: 56/60 px.
- Radio de borde: `0`; espaciado base: 4, 8, 12, 16, 24, 32 y 48 px.

## Estructura y componentes observados

- Barra de navegación superior de 56 px, fondo carbón, marca a la izquierda, pestañas horizontales y utilidades a la derecha. La navegación puede desplazarse horizontalmente.
- Barra de búsqueda debajo de la navegación, sobre superficie clara.
- Título de pantalla, descripción y acciones principales alineados en una cabecera de página.
- KPIs en cuadrícula, con valores grandes y separadores oscuros de un píxel.
- Secciones delimitadas por una regla superior de 2 px y un título.
- Tablas de contenido amplio con encabezados en Mono, bordes finos, desplazamiento horizontal y fila seleccionada marcada mediante fondo gris e inserto carbón.
- Botón primario carbón con texto claro; secundario delineado; acción ligera subrayada; acción destructiva delineada en óxido. Las formas son cuadradas.
- Badges cuadrados, compactos y en mayúsculas; estados de flujo representados también por segmentos.
- Campos con fondo gris elevado, borde oscuro, etiqueta y mensaje de ayuda/error.
- Drawer de detalle desde la derecha, con ancho máximo de 560 px, borde izquierdo de 2 px, encabezado, cuerpo desplazable y pie de acciones.
- Estados de foco visibles con contorno; controles deshabilitados con opacidad reducida.

## Adaptación observada

- Hasta 1280 px: se reduce la cantidad de columnas de KPI y se oculta el nombre de organización en la barra superior.
- Hasta 1100 px: las columnas principales pasan a una sola columna.
- Hasta 760 px: navegación superior con scroll, padding reducido, KPIs apilados y datos auxiliares en una columna.
- El drawer ocupa el ancho disponible en móvil y reduce su padding.

Estos breakpoints describen la exportación de referencia y son puntos de partida; se ajustarán al implementar las pantallas reales.

## Límites de la referencia

La exportación contiene datos de demostración, nombres, importes, etiquetas y acciones de ejemplo. No se consideran evidencia del comportamiento del producto actual ni reemplazan el inventario del informe `01-aplicacion-actual-y-paridad.md`. La lista de módulos, los campos, botones y flujos funcionales se mantienen anclados en ese informe, según la decisión del usuario.

## Aplicación en el frontend

Implementar los valores como tokens centrales del tema y componentes reutilizables. Los ajustes visuales propios de cada módulo deben limitarse a configuración admitida por el core; no deben permitir que un módulo rompa la consistencia de la paleta, la tipografía o los estados accesibles. Documentar cualquier cambio respecto a esta referencia antes de aplicarlo a toda la interfaz.
