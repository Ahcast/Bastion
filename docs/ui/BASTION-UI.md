# Bastion · Guía de implementación del estilo visual y despliegue

**Dirección visual:** *Concreto* (brutalismo gris mate)
**Aplica a:** repositorio `Ahcast/Bastion`, con frontend Angular 22 en Bluehost y backend Go en AWS Lightsail.
**Base de código revisada:** `main` @ `d74570a` (13 oct 2026).
**Paquete:** este archivo viaja dentro de `bastion-ui-kit.zip` junto con los estilos, logos, íconos, fuentes, componentes Angular y un parche listo para aplicar.

Esta guía es la especificación única de la interfaz. Va dirigida a quien implemente, sea persona o agente. Si una pantalla contradice esta guía, manda la guía. Si la guía no cubre un caso, se aplican sus cinco reglas (sección 1) y se documenta la decisión en `docs/development/decisions.md`.

---

## 0. Contenido del paquete

```
bastion-ui-kit/
├── BASTION-UI.md                      ← esta guía
├── styles/
│   ├── bastion.css                    ← tokens + fuentes + componentes bs-* (cargar primero)
│   ├── bastion-base.css               ← capa base: estiliza HTML sin clase y clases actuales
│   └── tokens.json                    ← tokens en formato de datos (fuente de verdad)
├── assets/
│   ├── brand/                         ← logos SVG (carbón, claro, apilado, símbolo, ícono, favicon)
│   ├── icons/                         ← 35 íconos SVG de trazo 24px (currentColor)
│   └── fonts/                         ← IBM Plex Sans 400/500/600 y Mono 400/500 (woff2) + licencia OFL
├── angular/
│   ├── bastion-concreto.patch         ← parche git probado sobre main @ d74570a
│   ├── shell.ts                       ← barra superior nueva (reemplaza core/shell/shell.ts)
│   ├── logo.ts                        ← componente <bs-logo>
│   └── icon.ts                        ← componente <bs-icon>
└── reference/
    ├── bastion-panel.html             ← prototipo navegable (Inicio + 11 secciones)
    ├── bastion-login.html             ← prototipo de acceso
    └── capturas/                      ← cómo se ve el parche aplicado sobre la app real
```

Los prototipos de `reference/` se abren en cualquier navegador. Muestran el comportamiento esperado: indicadores que actúan como selector, filas que se abren en línea y filtros. Son referencia visual, no código de producción.

---

## 1. Principios (no negociables)

1. **Gris y carbón.** El fondo es concreto (`--canvas`). Texto, cifras y acciones van en carbón (`--ink` #25292D). Todo texto, incluso el secundario, cumple contraste ≥ 7:1. No se usa blanco puro ni negro puro.
2. **Estructura a la vista.** Una sección se marca con una regla de carbón de 2px y un título. Una tabla, con una regla de 1px bajo el encabezado. Sin tarjetas con borde redondeado, sin sombras, **radio 0 en todo**.
3. **Masas de carbón para lo importante.** Son bloques sólidos de carbón: la barra superior, el botón principal, el indicador seleccionado, la prioridad alta y el filtro activo. Todo lo demás es gris.
4. **Un solo color de alerta.** Óxido (`--signal`) para lo que vence, falla o es error. Verde oscuro (`--ok`) solo para variaciones positivas y estados finales buenos (Adjudicado, Pagada). Ningún color decorativo.
5. **Nada se abre encima del trabajo.** Los datos no se muestran en ventanas emergentes ni paneles laterales flotantes. El detalle se abre **dentro de la tabla, debajo de la fila**. La única superficie flotante permitida es un menú desplegable corto (usuario, acciones).

Bastion es un motor de negocios: la meta es eficiencia. Si un elemento no ayuda a decidir o a actuar, sobra.

---

## 2. Instalación en el frontend

### Opción A: aplicar el parche (recomendada)

Probado sobre `main @ d74570a`: compila y pasa las 171 pruebas unitarias.

```bash
cd Bastion
git apply --check ruta/bastion-ui-kit/angular/bastion-concreto.patch
git apply ruta/bastion-ui-kit/angular/bastion-concreto.patch

# Fuentes: el parche no lleva archivos binarios
mkdir -p frontend/public/assets/fonts
cp ruta/bastion-ui-kit/assets/fonts/* frontend/public/assets/fonts/

# Opcional: logos e íconos como archivos (para PDF, correo, documentación)
mkdir -p frontend/public/assets/brand
cp ruta/bastion-ui-kit/assets/brand/*.svg frontend/public/assets/brand/

cd frontend && npm ci && npx ng build && npx ng test --watch=false
```

El parche hace estos cambios:

| Archivo | Cambio |
| --- | --- |
| `src/styles/bastion.css` | **Nuevo.** Tokens, `@font-face` locales y todos los componentes `bs-*`. |
| `src/styles.css` | **Reemplazado** por la capa base Concreto (antes: tema azul con menú lateral). |
| `angular.json` | Carga `src/styles/bastion.css` antes de `src/styles.css`. |
| `src/index.html` | Favicon SVG, `theme-color` y precarga de la fuente principal. |
| `src/app/core/ui/logo.ts` | **Nuevo** componente `<bs-logo>` (SVG en línea, hereda color). |
| `src/app/core/ui/icon.ts` | **Nuevo** componente `<bs-icon>` (35 íconos). |
| `src/app/core/shell/shell.ts` | Barra superior horizontal de carbón; el logo es Inicio y Configuración es un engranaje. |
| `src/app/core/auth/login.ts` | Encabezado discreto: símbolo + “BASTION”; se quita la frase descriptiva. |

**Compatibilidad con las pruebas e2e.** Se conservan los nombres accesibles que usan las pruebas de Playwright: enlaces “Inicio”, “Clientes”, “Proveedores”, “Órdenes de Compra”, “Facturas”, “Reportes”, “Colaboración” y “Configuración”, y los botones “Cerrar sesión”, “Ingresar” y “Verificar”, así como las etiquetas “Correo” y “Contraseña”. “Inicio” es el `aria-label` del logo y “Configuración” el del engranaje. Si se cambia “Órdenes de Compra” por “Órdenes de compra” (forma correcta en español), hay que actualizar `e2e/purchase-orders.spec.ts`, líneas 63 y 66.

### Opción B: manual

1. Copiar `styles/bastion.css` a `frontend/src/styles/bastion.css`.
2. Reemplazar el contenido de `frontend/src/styles.css` con `styles/bastion-base.css`.
3. En `angular.json` → `projects.*.architect.build.options.styles`, dejar `["src/styles/bastion.css", "src/styles.css"]`.
4. Copiar `assets/fonts/*` a `frontend/public/assets/fonts/` y `assets/brand/favicon.svg` a `frontend/public/assets/brand/`.
5. Copiar `angular/logo.ts` y `angular/icon.ts` a `frontend/src/app/core/ui/`.
6. Reemplazar `frontend/src/app/core/shell/shell.ts` con `angular/shell.ts`.
7. En `index.html`, dentro de `<head>`:
   ```html
   <meta name="theme-color" content="#25292d" />
   <link rel="icon" type="image/svg+xml" href="/assets/brand/favicon.svg" />
   <link rel="preload" href="/assets/fonts/ibm-plex-sans-latin-400-normal.woff2" as="font" type="font/woff2" crossorigin />
   ```
8. Aplicar el encabezado de acceso de la sección 9.

### Reglas de instalación

- **No cargar Google Fonts.** La CSP de `public/.htaccess` solo permite estilos y fuentes de `'self'`; las fuentes van autoalojadas en `/assets/fonts/`.
- **Un solo tema.** Bastion no tiene modo oscuro. Hay que eliminar las reglas `:root[data-theme="dark"]` (el parche ya las quita de `styles.css`) y ocultar o retirar la preferencia de tema en Configuración → Personal.
- **Estilos por componente.** Los estilos de cada componente se limitan a la disposición. Colores, tipografía y espacios salen siempre de los tokens (`var(--…)`); nunca se escriben valores hexadecimales dentro de un componente.

---

## 3. Tokens

La fuente de verdad es `styles/tokens.json`, y `bastion.css` la expone como variables CSS.

### 3.1 Color

| Token | Valor | Uso |
| --- | --- | --- |
| `--canvas` | `#ECECEC` | Fondo de página. La pestaña activa toma este color y “se funde” con la página. |
| `--panel` | `#F8FFFF` | Encabezados de tabla, franja de indicadores, fila de detalle, bandas de aviso. |
| `--panel-raised` | `#D2D6D6` | Campos de formulario, fila abierta o seleccionada. |
| `--line` | `#9AA2A3` | Línea de 1px entre filas y entre datos. |
| `--line-strong` | `#404649` | Reglas secundarias, marcos de controles. |
| `--ink` | `#25292D` | Carbón: texto, cifras, botones principales, barra superior, reglas de sección. |
| `--ink-muted` | `#2F3438` | Texto secundario en párrafos. |
| `--ink-subtle` | `#3C4246` | Rótulos Mono (encabezados de columna, etiquetas de campo). |
| `--accent` | `#25292D` | Igual a `--ink`: la acción es carbón. |
| `--accent-strong` | `#3A3F44` | Hover de masas de carbón (botón principal, pestaña del logo). |
| `--on-accent` | `#EEF0F0` | Texto e íconos sobre carbón. |
| `--accent-dim` | `rgba(37,41,45,.07)` | Hover de filas, chips y botones secundarios. |
| `--signal` | `#852A12` | Óxido: vencido, error, rechazo, prioridad crítica, “Requiere acción”. |
| `--signal-dim` | `rgba(156,50,22,.12)` | Fondo de banda de error y hover de botón peligroso. |
| `--ok` | `#174D2E` | Variación positiva, Adjudicado, Pagada, Aprobada. |
| `--chart-1…4` | `#34434B`, `#4D565A`, `#7F888B`, `#A3ABAC` | Datos: 1 = valor destacado; 3 y 4 = contexto y pistas de barras. |

**Mapeo de estados de negocio (obligatorio):**

| Estado | Presentación |
| --- | --- |
| Borrador, Pendiente, Abierto | `bs-st bs-st-open` (cuadro vacío, carbón) |
| En revisión, Enviada, Emitida, En curso | `bs-st` (cuadro carbón) |
| Aprobada, Adjudicado, Pagada, Entregada | `bs-st bs-st-ok` (verde) |
| Vencida, Rechazada, Error, Anulada con conflicto | `bs-st bs-st-alert` (óxido) |
| Anulada, Archivada, En papelera, Descartado | `bs-st bs-st-muted` (gris) |
| Prioridad alta | `bs-badge bs-badge-solid` |
| Crítico o vence hoy | `bs-badge bs-badge-alert` |

### 3.2 Tipografía

- **IBM Plex Sans** para lectura. **IBM Plex Mono** para rótulos, códigos de proceso, números de documento, fechas, horas, RTN y unidades.
- Las cifras siempre son tabulares (`font-variant-numeric: tabular-nums`, ya global) y se alinean a la derecha en tablas.

| Clase o token | Tamaño / interlínea | Peso | Uso |
| --- | --- | --- | --- |
| `.readout` | 56 / 60 | 500, −0.03em | Cifra única dominante (raro). |
| `.kpi` | 34 / 40 | 500, −0.02em | Valor de indicador. |
| `.h1` / `h1` | 32 / 40 | 500, −0.02em | Título de página (uno por pantalla). |
| `.h2` / `h2` | 18 / 24 | 600 | Título de sección. |
| `.body` | 16 / 24 | 400 | Texto base y celdas de tabla. |
| `.body-sm` | 14 / 20 | 400 | Notas, subtítulos de celda, ayudas. |
| `.label` | 12 / 16 Mono | 500, +0.05em, MAYÚSCULAS | Rótulos de campo y encabezados de columna. |
| `.code` | 14 / 20 Mono | 400, +0.02em | Códigos, RTN, números de documento. |

**Legibilidad primero:** el texto base es de 16px, ningún texto baja de 12px y solo los rótulos en mayúsculas usan 12px. Las celdas de tabla usan el mismo tamaño que el texto corrido.

### 3.3 Espacio, radio y sombra

- Retícula de 4px: `--space-1` 4 · `--space-2` 8 · `--space-3` 12 · `--space-4` 16 · `--space-5` 24 · `--space-6` 32 · `--space-7` 48.
- Márgenes de página: 32px en escritorio y 16px en teléfono. Ancho máximo de contenido: 1600px.
- Separación vertical entre bloques de página: 32px. Entre columnas: 40px.
- `--radius: 0` siempre.
- `--shadow-float` solo en menús desplegables.

---

## 4. Logo

**Logo vigente (provisional aprobado):** la planta de una fortaleza abaluartada. Tiene una muralla con cinco baluartes en diamante, un recinto pentagonal con nodos en los vértices y un pentágono al centro. Es de una sola tinta y de líneas gruesas. Nombre: “BASTION” en IBM Plex Sans SemiBold, mayúsculas, +0.24em, convertido a trazos.

| Archivo | Uso |
| --- | --- |
| `bastion-lockup.svg` | Principal, carbón sobre concreto o papel: documentos, proformas, firmas. |
| `bastion-lockup-light.svg` | Claro sobre carbón: portadas oscuras. |
| `bastion-lockup-stacked.svg` / `-light` | Símbolo arriba y nombre debajo: acceso, portadas, presentaciones. |
| `bastion-mark.svg` / `-light` | Solo el símbolo, cuando el nombre ya está visible. |
| `bastion-mark-currentcolor.svg` | Símbolo que hereda el color del texto (para incrustar). |
| `bastion-appicon.svg`, `favicon.svg` | Favicon e ícono de app: símbolo claro sobre cuadrado de carbón. |

**En Angular, siempre `<bs-logo>`** (SVG en línea, toma el color de CSS):

```html
<bs-logo />                                  <!-- lockup, 28px de alto, carbón -->
<bs-logo variant="mark" [size]="56" />       <!-- solo símbolo -->
<bs-logo variant="stacked" [size]="120" />   <!-- apilado -->
<bs-logo tone="inverse" />                   <!-- claro, para fondos de carbón -->
```

Dentro de `.bs-topnav` el logo se invierte solo, y vuelve a carbón cuando Inicio está activo.

**Reglas del logo:**
- Rinde mejor desde 24px de alto. Tamaños mínimos: lockup de 96px de ancho y símbolo de 16px (favicon).
- Zona de respeto: la mitad del alto del símbolo por cada lado.
- No se colorea fuera de carbón o claro, no se contornea, no lleva sombras ni efectos, no se deforma ni se rota.
- **PDF de cotización y factura:** el backend acepta un PNG local en `BASTION_PDF_LOGO` (≤ 1 MiB, ≤ 2000×2000). Hay que exportar `bastion-lockup.svg` a PNG de 1200px de ancho con fondo transparente, por ejemplo con `rsvg-convert -w 1200 bastion-lockup.svg -o bastion-lockup.png`.

---

## 5. Estructura de pantalla y menú

### 5.1 Mapa de Bastion (orden aprobado)

**Inicio** (el logo) · Clientes · Proveedores · Procesos · Cotizaciones · Órdenes de compra · Facturas · Gastos · Reportes · Colaboración · **Configuración** (el engranaje).

| Sección | Ruta | Permiso | Módulo |
| --- | --- | --- | --- |
| Inicio | `/` | (sesión) | — |
| Clientes | `/customers` | `clientes.view` | `customers` |
| Proveedores | `/suppliers` | `proveedores.view` | `suppliers` |
| Procesos | `/processes` | `procesos.view` | `processes` |
| Cotizaciones | `/quotes` | `cotizaciones.view` | `quotes` |
| Órdenes de compra | `/purchase-orders` | `ordenes.view` | `purchase-orders` |
| Facturas | `/invoices` | `facturas.view` | `invoices` |
| Gastos | `/expenses` | `gastos.view` | `expenses` |
| Reportes | `/reports` | `reportes.view` | `reports` |
| Colaboración | `/collaboration` | `colaboracion.view` | `collaboration` |
| Configuración | `/settings` | (sesión) | — |

IA y Workflow son transversales y **no llevan pestaña** (regla 4 de `AGENTS.md`). Una pestaña solo aparece si el usuario tiene el permiso y el módulo está habilitado; esa lógica la conserva `angular/shell.ts`.

### 5.2 Barra superior (`.bs-topnav`, 56px)

```
┌──────────────┬─────────────────────────────────────────────────────┬──────────────────────┐
│ [▣ BASTION]  │ Clientes  Proveedores  Procesos  Cotizaciones  …     │ 🔔  ⚙  [OC]  ⇥       │
└──────────────┴─────────────────────────────────────────────────────┴──────────────────────┘
  logo = Inicio   pestañas de texto 14px, sin íconos                  alertas · config · usuario · salir
```

- **Fondo:** carbón (`--ink`), sticky arriba.
- **Logo = Inicio:** bloque de 24px de padding a cada lado. Cuando Inicio está activo, el bloque toma el concreto y el logo pasa a carbón (`.bs-topnav-home.bs-tab-active`).
- **Pestañas** (`.bs-tab`): 14px, peso 500, color `#C4CACB`, hover `--on-accent`. La activa toma el fondo `--canvas` y el texto carbón, como una puerta abierta en la masa. Se marca con `aria-current="page"`.
- **Contador** (`.bs-tab-count`, Mono 13px): solo para pendientes que requieren acción, por ejemplo “Procesos 37”. No se muestra para totales.
- **Herramientas** a la derecha, en este orden:
  1. **Búsqueda** (`search`), solo cuando exista un endpoint de búsqueda global. Abre `.bs-searchbar` debajo de la barra, no encima.
  2. **Alertas** (`bell`). Abre la franja de notificaciones debajo de la barra y muestra `.bs-dot` óxido si hay no leídas.
  3. **Configuración** (`settings`), como pestaña-ícono que se marca activa igual que las demás.
  4. **Usuario:** cuadro de concreto con iniciales en Mono.
  5. **Cerrar sesión** (`logout`).
- Todos los botones de solo ícono llevan `aria-label` y `title`.
- **Ancho:** desde 1440px caben en una línea. Más angosto, las pestañas se desplazan de lado sin barra visible. En teléfono (≤ 760px) la barra pasa a dos filas: arriba logo y herramientas, abajo las pestañas desplazables.

### 5.3 Página tipo

```html
<main class="bs-page">                       <!-- lo pone el shell -->
  <header class="bs-page-head">
    <div>
      <span class="bs-label">Sábado 10 oct · actualizado 06:42</span>
      <h1>Cotizaciones</h1>
    </div>
    <div class="bs-actions">
      <button class="bs-btn bs-btn-secondary">Exportar</button>
      <button class="bs-btn bs-btn-primary"><bs-icon name="plus" />Nueva cotización</button>
    </div>
  </header>

  <div class="bs-kpis bs-kpis-4" role="tablist"> … indicadores … </div>
  <div class="bs-two"> … tendencia y desglose … </div>
  <section class="bs-frame"> … tabla de trabajo … </section>
</main>
```

| Pantalla | Composición |
| --- | --- |
| **Inicio** | Título; **cinco indicadores** que actúan como selector (Procesos, Cotizaciones, Órdenes de compra, Por cobrar, Gastos). Debajo, una fila con *Requiere acción* + tendencia + desglose del módulo elegido (`.bs-row`) y la **tabla de trabajo a todo el ancho**. |
| **Cada módulo** | Título y acciones; cuatro indicadores (`.bs-kpis-4`); tendencia y desglose (`.bs-two`); tabla de trabajo con filtros y detalle en línea. |
| **Configuración** | Sin gráficos. Secciones `.bs-frame` (Personal, Usuarios y roles, Empresa y SMTP, Archivos, Servicios, Auditoría), cada una con su formulario o tabla. |

---

## 6. Componentes

Cada componente es HTML con clases `bs-*` de `bastion.css`. No se necesitan librerías de UI.

### 6.1 Botones

```html
<button class="bs-btn bs-btn-primary">Guardar cotización</button>   <!-- 1 por vista: la acción principal -->
<button class="bs-btn bs-btn-secondary">Exportar</button>
<button class="bs-btn bs-btn-ghost">Ver historial</button>          <!-- acción terciaria, subrayada -->
<button class="bs-btn bs-btn-danger">Anular factura</button>        <!-- destructiva, óxido -->
<button class="bs-btn bs-btn-primary bs-btn-sm">…</button>          <!-- en tablas y detalle -->
<button class="bs-btn bs-btn-primary" aria-busy="true" disabled>Guardando…</button>
```

- Texto en infinitivo o sustantivo: “Guardar cotización”, “Nueva orden”, “Registrar gasto”. Nada de “OK” ni “Enviar” a secas.
- **Una sola acción principal visible por zona.**
- Mientras una operación está en curso, el botón queda deshabilitado y su texto cambia a gerundio (“Guardando…”), lo que evita dobles envíos. El backend ya exige `Idempotency-Key`.

### 6.2 Campos de formulario

```html
<form class="bs-form">
  <fieldset class="bs-fieldset">
    <legend>Datos del cliente</legend>
    <div class="bs-form-grid">
      <div class="bs-field">
        <label for="c-name">Nombre o razón social<span class="bs-req" aria-hidden="true">*</span></label>
        <div class="bs-input-wrap"><input id="c-name" class="bs-input" required /></div>
      </div>
      <div class="bs-field">
        <label for="c-rtn">RTN</label>
        <div class="bs-input-wrap"><input id="c-rtn" class="bs-input bs-mono" inputmode="numeric" maxlength="14" /></div>
        <p class="bs-field-msg">14 dígitos, sin guiones.</p>
      </div>
      <div class="bs-field bs-field-error">
        <label for="c-mail">Correo</label>
        <div class="bs-input-wrap"><input id="c-mail" class="bs-input" type="email" aria-invalid="true" aria-describedby="c-mail-e" /></div>
        <p id="c-mail-e" class="bs-field-msg bs-field-msg-error">Escriba un correo válido.</p>
      </div>
      <div class="bs-field">
        <label for="c-cur">Moneda</label>
        <div class="bs-input-wrap"><select id="c-cur" class="bs-select"><option>HNL</option><option>USD</option></select></div>
      </div>
      <div class="bs-field">
        <label for="c-total">Monto</label>
        <div class="bs-input-wrap"><span class="bs-input-prefix">L</span><input id="c-total" class="bs-input bs-num" inputmode="decimal" /></div>
      </div>
      <div class="bs-field bs-span-2">
        <label for="c-notes">Notas</label>
        <div class="bs-input-wrap"><textarea id="c-notes" class="bs-textarea"></textarea></div>
      </div>
      <label class="bs-check"><input type="checkbox" /> Cliente activo</label>
    </div>
  </fieldset>
  <div class="bs-form-foot">
    <button type="button" class="bs-btn bs-btn-secondary">Cancelar</button>
    <button type="submit" class="bs-btn bs-btn-primary">Guardar cliente</button>
  </div>
</form>
```

**Reglas de campos:**
- **Rótulo siempre visible** arriba del campo, en Mono 12px mayúsculas. El *placeholder* nunca reemplaza al rótulo; se usa solo para un ejemplo (“0801-1990-12345”).
- Campo: fondo `--panel-raised`, borde de 1px carbón, radio 0. Al enfocar, contorno de 2px carbón.
- **Obligatorio:** asterisco óxido. Lo opcional no se marca.
- **Error:** borde y mensaje en óxido debajo del campo, con `aria-invalid="true"` y `aria-describedby`. El mensaje dice qué hacer (“Escriba un correo válido”), no solo qué falló. Al enviar con errores, el foco va al primer campo inválido.
- **Números y montos:** Mono, alineados a la derecha, con prefijo de moneda (`L`, `US$`). Los montos viajan como **texto decimal**: nunca se convierten a `number` de JavaScript (regla 7 de `AGENTS.md`).
- **Fechas:** `type="date"` (Mono). Se muestran como `12 OCT 2026`; las horas, como `06:42`.
- **Solo lectura:** `.bs-field-readonly` (fondo transparente, borde `--line`). **Deshabilitado:** borde punteado.
- **Casillas y opciones:** cuadradas (`.bs-check`).
- **Distribución:** dos columnas en escritorio (`.bs-form-grid`) y una en teléfono. Los grupos se separan con `<fieldset class="bs-fieldset">`, cuya leyenda lleva la regla de 2px.
- **Pie:** “Cancelar” secundario a la izquierda de la acción principal, alineados a la derecha (`.bs-form-foot`).

La capa base (`bastion-base.css`) ya da este aspecto a los `<form>`, `<label>`, `<input>`, `<select>` y `<textarea>` actuales **sin cambiar las plantillas**. Un `<label>` que envuelve el campo muestra el rótulo arriba. Un botón de envío dentro de un formulario sale como principal; los demás, como secundarios.

### 6.3 Filtros (chips)

```html
<div class="bs-filters" role="group" aria-label="Filtrar por estado">
  <button class="bs-chip" aria-pressed="true">Todas<b>24</b></button>
  <button class="bs-chip" aria-pressed="false">Borrador<b>6</b></button>
  <button class="bs-chip" aria-pressed="false">En revisión<b>3</b></button>
</div>
```

El filtro activo es una masa de carbón. Los filtros se reflejan en la URL (`?estado=`), que es como ya funcionan los módulos.

### 6.4 Indicadores = selector

```html
<div class="bs-kpis" role="tablist" aria-label="Módulo de la tabla">
  <button class="bs-kpi-tile" role="tab" aria-selected="true" aria-controls="tabla">
    <span class="bs-label">Procesos activos</span>
    <span class="bs-kpi-value">37</span>
    <span class="bs-kpi-foot"><span class="bs-kpi-delta bs-kpi-up">▲ 6</span> vs. semana anterior</span>
  </button>
  …
</div>
```

- Al hacer clic, **la tabla de trabajo cambia a ese módulo** y la fila de arriba muestra su tendencia y desglose. No se abre ningún panel.
- El indicador seleccionado es carbón (`aria-selected="true"`).
- Las flechas del teclado mueven la selección (patrón *tablist*).
- Para indicadores informativos que no seleccionan nada, usar `.bs-kpi-tile.bs-static` (un `<div>`, no un botón).

### 6.5 Tabla de trabajo con detalle en línea

```html
<section class="bs-frame" id="tabla">
  <div class="bs-frame-head"><h2 class="bs-frame-title">Cotizaciones</h2> <div class="bs-filters">…</div></div>
  <div class="bs-table-wrap">
    <table class="bs-table">
      <thead><tr><th></th><th>Número</th><th>Cliente</th><th>Estado</th><th class="bs-num">Total</th><th>Vence</th></tr></thead>
      <tbody>
        <tr tabindex="0" aria-expanded="true" aria-controls="xp-412">
          <td class="bs-caret">−</td>
          <td class="bs-mono">COT-0412</td>
          <td><span class="bs-cell-title">Hospital Escuela</span><span class="bs-cell-sub">LPN-2026-118 · 3 partidas</span></td>
          <td><span class="bs-st">En revisión</span></td>
          <td class="bs-num">L 1,284,500.00</td>
          <td class="bs-mono">14 OCT</td>
        </tr>
        <tr class="bs-xp-row" id="xp-412">
          <td colspan="6">
            <div class="bs-xp">
              <div class="bs-xp-col">
                <div class="bs-facts"> <div><span class="bs-label">Subtotal</span><span class="bs-fv bs-mono">L 1,116,956.52</span></div> … </div>
                <div class="bs-sec"><h3>Partidas</h3><table class="bs-mini">…</table></div>
              </div>
              <div class="bs-xp-col">
                <div class="bs-sec"><h3>Flujo del documento</h3><ul class="bs-timeline">…</ul></div>
                <div class="bs-xp-actions">
                  <button class="bs-btn bs-btn-secondary bs-btn-sm">Abrir proceso relacionado</button>
                  <button class="bs-btn bs-btn-primary bs-btn-sm">Aprobar</button>
                </div>
              </div>
            </div>
          </td>
        </tr>
        <tr tabindex="0" aria-expanded="false"> <td class="bs-caret">+</td> … </tr>
      </tbody>
    </table>
  </div>
  <div class="bs-table-foot"><span class="bs-muted">24 cotizaciones · página 1 de 1</span><div class="bs-pager">…</div></div>
</section>
```

- **Clic o Enter en una fila:** se abre su detalle debajo, dentro de la tabla (`+` / `−`). Otro clic lo cierra. **Solo una fila abierta a la vez.**
- **Contenido del detalle:**
  - Proceso: partidas, precios de proveedores y cronología.
  - Cotización, orden, factura o gasto: datos clave (`.bs-facts`), el proceso relacionado y el flujo del documento.
  - Las acciones de la fila van en `.bs-xp-actions`.
- **Proceso relacionado = salto:** el enlace cambia la tabla al módulo Procesos con esa fila abierta (`/processes?abrir=<id>`).
- **Celdas:**
  - Primera columna de texto: `.bs-cell-title` + `.bs-cell-sub`.
  - Códigos y fechas: `.bs-mono`.
  - Montos: `.bs-num`, con la moneda siempre visible.
- **Filas de una línea.** Nada de párrafos dentro de las celdas.
- **Paginación:** 25 por página, el valor por defecto del API; el pie muestra el total.
- Las tablas sin clase de las pantallas actuales ya reciben este estilo por la capa base. Para el detalle en línea hay que migrar la tabla a `.bs-table`.

### 6.6 Secciones, listas y datos

- **Sección:** `<section class="bs-frame"><div class="bs-frame-head"><h2 class="bs-frame-title">…</h2></div>…</section>`. La clase actual `.card` se ve igual gracias a la capa base.
- **Datos clave:** `.bs-facts`, una cuadrícula de 2 columnas con rótulo Mono y valor de 17px.
- **Lista “Requiere acción”:** `.bs-list > button.bs-item` con `.bs-item-t` (qué), `.bs-item-s` (contexto) y `.bs-item-go` (“Revisar →”). Lo urgente lleva `.bs-item-alert` (título en óxido). Cada pendiente cambia la tabla al módulo correcto y abre la fila exacta.
- **Cronología:** `.bs-timeline`, con la fecha en Mono a la izquierda.
- **Avance por etapas:** `.bs-stage` con segmentos `.bs-seg` / `.bs-seg-on`, más las variantes `-done`, `-alert` y `-off`.
- **Gráficos:** barras simples `.bs-bars` (la barra destacada `.bs-hi` va en carbón y el resto en `--chart-4`) y barras horizontales `.bs-hbars` para desgloses. Sin leyendas de colores, sin 3D, sin gradientes; las cifras se escriben sobre las barras en Mono.

### 6.7 Estados de la interfaz

| Estado | Patrón |
| --- | --- |
| Cargando | `.bs-skeleton` con la forma de las filas; nunca un spinner a pantalla completa. |
| Vacío | `.bs-state`: título (“No hay cotizaciones con este filtro”) + una acción (“Quitar filtros” o “Nueva cotización”). |
| Error de carga | `.bs-band.bs-band-alert` encima de la tabla, con `role="alert"`, mensaje y “Reintentar”. |
| Operación completada | `[role="status"]` (banda con borde verde) arriba del bloque afectado. Desaparece al siguiente cambio; **sin notificaciones flotantes** (*toasts*). |
| Confirmación destructiva | `.bs-band.bs-band-alert` **en línea**, encima del formulario o dentro del detalle de la fila: “¿Anular la factura F-0098? Esta acción no se puede deshacer.” + [Cancelar] [Anular factura]. |
| Reautenticación (MFA) | El mismo patrón en línea, con los campos Contraseña y Código, en el lugar de la acción que la pide. |

---

## 7. Íconos

- 35 íconos de trazo, 24px de cuadrícula, trazo 1.75, `currentColor`, compatibles con Lucide.
- Se usan con `<bs-icon name="…" />`; los archivos están en `assets/icons/`.
- **Navegación y herramientas:** `dashboard`, `clients`, `suppliers`, `tenders` (procesos), `quotes`, `orders`, `invoices`, `expenses`, `reports`, `team` (colaboración), `settings`, `search`, `bell`, `logout`.
- **Acciones:** `plus`, `edit`, `copy`, `trash`, `download`, `upload`, `refresh`, `filter`, `check`, `x`, `eye`, `eye-off`, `file`, `mail`, `clock`.
- **Indicadores:** `arrow-up`, `arrow-down`, `alert`, `chevron-down`, `chevron-right`, `sparkle` (marca de IA).

**Reglas de uso:**
- Ícono de 20px en botones y 18px en campos, en `--ink` o `--ink-subtle`; claro sobre la barra de carbón.
- **Las pestañas no llevan ícono.** Los íconos acompañan acciones; no decoran títulos.
- Un ícono sin texto visible siempre lleva `aria-label` y `title`.

---

## 8. Voz y formato de datos

- **Idioma:** español de Honduras. La interfaz usa infinitivo o sustantivo (“Nueva cotización”, “Revisar”); los documentos externos tratan de usted.
- **Brevedad:** “Cierra en 2 días”, “Sin respuesta > 48 h”, “3 partidas sin precio”.
- **Moneda:** `L 1,284,500.00` en tablas y documentos; resúmenes como `L 4.82 M`. USD se escribe `US$ 12,400.00`. Nunca se mezclan monedas en un total; cada moneda va en su propia línea.
- **Fechas y horas:** en Mono, `12 OCT 2026` y `06:42`. Las fechas relativas solo se usan en cronologías (“hace 2 h”). La hora local es América/Tegucigalpa (UTC−6).
- **Identificadores:** códigos de proceso, número de documento y RTN en Mono, tal como vienen del origen.
- **IA:** no habla en primera persona. Se escribe “IA clasificó 6 procesos”, nunca “Clasifiqué…”, y su marca es el ícono `sparkle`.
- **Errores:** dicen qué pasó y qué hacer, sin culpar ni usar códigos técnicos. El `request_id` va en texto pequeño Mono para soporte.

---

## 9. Acceso (login)

Debe ser discreto: no revela qué es ni a quién pertenece.

```
            ▣              ← símbolo, 56px
         B A S T I O N      ← 18px, 600, +0.24em
     ┌───────────────────┐
     │ CORREO            │
     │ CONTRASEÑA      👁 │
     │ [    Ingresar    ] │
     └───────────────────┘
```

- Lleva solo símbolo + nombre, Correo, Contraseña (con botón `eye` para mostrar) e Ingresar. **No lleva** eslogan, nombre de empresa, “¿olvidó su contraseña?”, registro ni pie con enlaces.
- **Error genérico único:** “No se pudo iniciar sesión.” No distingue si el correo existe; el backend ya responde igual en ambos casos.
- **Pasos siguientes:** verificación TOTP y códigos de recuperación, en la misma columna de 340px y con el mismo estilo.
- `<title>` de la página: “Acceso”.
- **Encabezado en `login.ts`** (incluido en el parche):
  ```html
  <section class="auth">
    <header class="bs-login-mark">
      <bs-logo variant="mark" [size]="56" />
      <h1 class="bs-login-name">Bastion</h1>
    </header>
    …formularios existentes…
  ```

---

## 10. Accesibilidad (criterios de aceptación)

- Contraste ≥ 7:1 en todo texto. Los tokens ya lo cumplen; no hay que inventar grises nuevos.
- Foco visible siempre: contorno de 2px carbón, o claro sobre la barra.
- Todo funciona con teclado: Tab, Enter y Espacio, y flechas en el selector de indicadores.
- `aria-current="page"` en la pestaña activa, `aria-selected` en los indicadores, `aria-expanded` en las filas y `aria-pressed` en los chips.
- Ningún texto por debajo de 12px; las áreas de clic miden al menos 40×40 en la barra.
- Se respeta `prefers-reduced-motion`; el *skeleton* deja de animarse.

---

## 11. Adaptación a pantalla

| Ancho | Comportamiento |
| --- | --- |
| ≥ 1440 | Barra en una línea; cinco indicadores; filas de tres columnas. |
| 1100 – 1440 | Las pestañas se desplazan de lado; tres indicadores por fila. |
| 760 – 1100 | Filas y detalle en una columna. |
| < 760 | Barra en dos filas; indicadores apilados; formularios a una columna; márgenes de 16px; tablas con desplazamiento horizontal. |

---

## 12. Despliegue

### 12.1 Antes de publicar (corregir en el backend)

**1. IP real detrás de nginx (bloqueante).**
- **Problema:** `internal/core/api/server.go` (líneas ~196 y ~335) usa `r.RemoteAddr`. Detrás de nginx eso siempre es `127.0.0.1`, así que los límites “por IP” se vuelven globales:
  - 60 intentos de acceso cada 15 minutos para toda la empresa: cualquiera puede bloquear el login de todos.
  - 3,000 peticiones cada 15 minutos para todos los usuarios juntos.
  - Además, todas las peticiones compiten por la misma fila bloqueada en `auth_limits`.
- **Corrección propuesta.** No está compilada en este análisis; validarla en CI con `go test -race`.
  ```nginx
  # en cada location del server de la API
  proxy_set_header X-Real-IP $remote_addr;
  ```
  ```go
  // server.go — aceptar X-Real-IP solo si la conexión viene de nginx local
  func clientIP(r *http.Request) string {
      host, _, err := net.SplitHostPort(r.RemoteAddr)
      if err != nil { host = r.RemoteAddr }
      if ip := net.ParseIP(host); ip != nil && ip.IsLoopback() {
          if real := net.ParseIP(strings.TrimSpace(r.Header.Get("X-Real-IP"))); real != nil {
              return real.String()
          }
      }
      return host
  }
  // usar clientIP(r) en "api-ip:" y "login-ip:"
  ```

**2. Limpieza de sesiones (importante).**
- **Problema:** cada `GET /api/v1/auth/csrf` sin sesión crea una fila en `sessions`, y nada borra las vencidas ni los registros viejos de `auth_limits`.
- **Corrección:** agregar al worker, que corre cada minuto, una limpieza acotada:
  ```sql
  DELETE FROM sessions WHERE expires_at < UTC_TIMESTAMP(6) LIMIT 1000;
  DELETE FROM sessions WHERE user_id IS NULL AND last_activity < UTC_TIMESTAMP(6) - INTERVAL 1 HOUR LIMIT 1000;
  DELETE FROM auth_limits WHERE window_start < UTC_TIMESTAMP(6) - INTERVAL 1 DAY LIMIT 1000;
  ```

**3. Crawler sobre HTTPS.** En `bastion_crawler/delivery.py`, rechazar `BASTION_API_URL` que no empiece con `https://` fuera de desarrollo.

### 12.2 Frontend (Bluehost)

1. Configurar `frontend/public/config.json`:
   ```json
   {"apiUrl":"https://api.bastion.castilloit.net"}
   ```
2. Compilar con Node ≥ 22.22.3 o 24.15:
   ```bash
   cd frontend && npm ci && npx ng build && npx ng test --watch=false
   ```
3. Subir **solo** el contenido de `frontend/dist/bastion-ui/browser/` a la raíz del subdominio (por ejemplo `bastion.castilloit.net`), incluyendo `.htaccess`, `config.json` y `assets/fonts/`. Nunca se suben `node_modules`, fuentes del proyecto ni secretos.
4. **CSP** (`public/.htaccess`): ya es compatible con este estilo. Fuentes y estilos vienen de `'self'`, y el ícono del `<select>` es un `data:` permitido por `img-src`. Solo hay que cambiar `connect-src` si la API usa otro dominio, y agregar el origen exacto del bucket S3 cuando se activen los archivos.
5. **Verificación:**
   - Abrir `/login` y comprobar en la pestaña Red que las fuentes cargan desde `/assets/fonts/` sin errores de CSP.
   - Iniciar sesión y recorrer las pestañas.
   - Confirmar que el logo lleva a Inicio y el engranaje a Configuración.

### 12.3 Backend (Lightsail)

1. **Compilar** con Go 1.27.2, la versión que exige `go.mod`:
   ```bash
   cd backend && go vet ./... && go test -race ./... && make build   # bin/server, bin/worker, bin/admin
   ```
2. **Instalar:**
   ```bash
   sudo useradd --system --home /opt/bastion --shell /usr/sbin/nologin bastion
   sudo install -d -o bastion -g bastion /opt/bastion/bin
   sudo install -o bastion -g bastion -m 0755 bin/server bin/worker bin/admin /opt/bastion/bin/
   sudo install -d -m 0750 /etc/bastion
   ```
3. **Configurar `/etc/bastion/core.env`** (permisos 600, propietario root), partiendo de `backend/.env.example`:
   ```ini
   BASTION_ENV=production
   BASTION_LISTEN=127.0.0.1:8080
   BASTION_FRONTEND_ORIGIN=https://bastion.castilloit.net
   BASTION_MYSQL_DSN=<usuario>:<clave>@tcp(<host>:3306)/<bd>?parseTime=true&loc=UTC&charset=utf8mb4&tls=true
   BASTION_DB_TUNNEL_VERIFIED=false          # true solo si se usa túnel SSH supervisado y verificado
   BASTION_ENCRYPTION_KEY=<salida de: openssl rand -base64 32>
   BASTION_PASSWORD_DENYLIST_FILE=/etc/bastion/password-denylist.sha256
   BASTION_PDF_LOGO=/etc/bastion/brand/bastion-lockup.png
   BASTION_EMAIL_ENABLED=false               # activar solo tras validar SMTP real
   BASTION_FILES_ENABLED=false               # activar solo tras validar S3/IAM y escáner
   BASTION_MCP_ENABLED=false
   ```
   - La clave de cifrado se respalda fuera del servidor: sin ella no se pueden leer los secretos TOTP guardados.
   - MySQL vive en Bluehost: conexión cifrada o túnel SSH verificado, y acceso restringido a la IP estática de Lightsail (ver `docs/operations.md`).
4. **Crear la base:**
   ```bash
   # systemd lee core.env como root y ejecuta como el usuario bastion
   sudo systemd-run --pty --wait --collect --uid=bastion -p EnvironmentFile=/etc/bastion/core.env /opt/bastion/bin/admin migrate
   sudo systemd-run --pty --wait --collect --uid=bastion -p EnvironmentFile=/etc/bastion/core.env /opt/bastion/bin/admin bootstrap
   # bootstrap es interactivo: correo, nombre y contraseña de 15+ caracteres (solo funciona con la base vacía)
   ```
   Respaldar la base antes de cada migración en producción.
5. **Servicios:**
   ```bash
   sudo cp deploy/bastion-server.service deploy/bastion-worker.service deploy/bastion-worker.timer /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now bastion-server bastion-worker.timer
   curl -s http://127.0.0.1:8080/readyz            # {"status":"ready"}
   ```
6. **nginx y TLS:**
   - Copiar `deploy/nginx.conf.example` a `/etc/nginx/sites-available/bastion-api`.
   - Agregar `proxy_set_header X-Real-IP $remote_addr;` en ambos `location` (ver 12.1).
   - Obtener el certificado con `certbot --nginx -d api.bastion.castilloit.net`.
   - Recargar nginx.
7. **Primer acceso:**
   - Entrar en `https://bastion.castilloit.net/login` con el usuario creado.
   - Enrolar TOTP y guardar los códigos de recuperación.
   - Crear roles y asignar permisos explícitos por módulo; ser administrador no da permisos comerciales.
8. **Crawler:** seguir `backend/crawler/deploy/INSTALL.md`.
   - Crear en Configuración → Servicios una identidad con los scopes `crawler.runs.create`, `crawler.processes.import` y `crawler.runs.complete`, con vigencia limitada.
   - Guardar el token en `/etc/bastion-crawler.env` (`BASTION_TOKEN`, `BASTION_API_URL=https://api.bastion.castilloit.net`).
   - Activar con `sudo systemctl enable --now bastion-crawler.timer bastion-retry.timer`.

### 12.4 Lista de verificación final

**Backend y seguridad**
- [ ] Correcciones 12.1 (IP real y limpieza) aplicadas y CI en verde: Go vet/race sobre MySQL 5.7, Angular build, pruebas unitarias y navegador.
- [ ] `core.env` con `BASTION_ENV=production`, origen HTTPS exacto, DSN cifrado y clave de cifrado respaldada.
- [ ] Respaldo de MySQL probado (crear y restaurar) antes de `admin migrate`.
- [ ] `/readyz` responde `ready`; nginx con TLS y `X-Real-IP`; HSTS activo.
- [ ] Prueba de límites: 3 intentos fallidos desde un equipo no bloquean el acceso desde otro.

**Frontend**
- [ ] Desplegado en Bluehost con `config.json` apuntando a la API HTTPS y sin errores de CSP en consola.
- [ ] Fuentes Plex cargando desde `/assets/fonts/`; favicon visible.

**Interfaz**
- [ ] Barra superior en carbón con el orden aprobado; el logo lleva a Inicio y el engranaje a Configuración; las pestañas aparecen según permisos.
- [ ] Formularios: rótulos Mono arriba, errores en óxido, montos en Mono a la derecha.
- [ ] Tablas: encabezado Mono, filas de una línea, detalle en línea (sin ventanas emergentes).
- [ ] Acceso discreto con error genérico; TOTP obligatorio.
- [ ] Revisión en 1440px, 1280px y teléfono (390px).

---

## 13. Hoja de ruta de migración por módulo

El parche cambia el estilo de toda la aplicación de una vez gracias a la capa base. Después, cada módulo se migra a los patrones de esta guía, uno a la vez y con sus pruebas:

1. **Inicio:** cinco indicadores como selector + “Requiere acción” + tabla de trabajo del módulo elegido. Reutilizar `HomeMetrics` como fuente de los indicadores.
2. **Procesos, Cotizaciones, Órdenes y Facturas:** tabla `.bs-table` con detalle en línea, chips de estado y cuatro indicadores.
3. **Gastos, Clientes y Proveedores:** tabla + ficha en línea con `.bs-facts`; los formularios de alta van en la misma página, debajo del encabezado, sin modal.
4. **Reportes:** filtros arriba, `.bs-bars` / `.bs-hbars` y la exportación Excel como acción secundaria.
5. **Colaboración:** lista `.bs-list` para tareas y menciones; la conversación usa `.bs-timeline`.
6. **Configuración:** secciones `.bs-frame` con sus formularios; quitar la preferencia de tema oscuro.

**Criterio de terminado por módulo:**
- La pantalla usa solo tokens y clases `bs-*`.
- Cumple las secciones 6 a 10.
- Las pruebas unitarias y e2e del módulo siguen en verde.
- Hay una captura a 1440px y otra a 390px en el PR o en el commit.
