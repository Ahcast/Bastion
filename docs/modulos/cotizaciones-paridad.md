# Cotizaciones: cotejo con la copia de CastilloIT

Revisión estática: 2026-10-22, sobre la copia indicada por el usuario en Downloads. No se ejecutó Laravel ni se leyeron manifiestos privados, credenciales, archivos de clientes o envíos reales.

## Fuentes activas

`index.html` carga `main-4ZA7RVJ2.js`, cuyas rutas apuntan al listado `chunk-WAOX4DBJ.js`, editor `chunk-OEB7YRWR.js` y PDF `chunk-GUMROS57.js`. Se cotejan con `back/routes/api.php` y `back/app/Http/Controllers/CotizacionesController.php`.

## Reglas comprobadas

- `nextQuoteNumber` (líneas 1689–1696) bloquea un contador e incrementa antes de asignar `COT-` más seis posiciones mínimas con ceros. No reinicia por año. Crear y clonar asignan número; editar lo conserva. En Bastion el contador es propio, comienza con datos nuevos y no presupone continuidad con producción; una futura migración requiere reconciliar la numeración.
- El editor activo inicia **30 días**, término `Tiempo de entrega: ` y un ítem de cantidad 1, precio 0, impuesto `isv15`. La ficha anterior indicaba 15 días: es el fallback del backend/lectura, no el default de alta de esta compilación.
- `valid_days` admite 0..3650. `valid_until` es fecha de cotización más días, sin restar uno. Los días cero conservan la fecha inicial.
- Estados visibles y aceptados por `updateStatus`: `Borrador`, `Enviada`, `Aprobada`, `Rechazada`, `Convertido`. No se observa matriz de transiciones. El formulario del backend permite cualquier texto; Bastion aplica la lista visible y no incorpora estados arbitrarios como requisito.
- Cliente manual y catálogo son opcionales. Fecha es obligatoria. Notas hasta 3000, términos hasta 5000, cliente hasta 255, referencia de orden hasta 100, descripción de ítem hasta 3000.
- Cantidades/precios/descuentos no negativos, normalizados a dos decimales por el backend; cantidad cero y descripción vacía se permiten. Una fila se descarta solo si descripción vacía, cantidad cero y precio cero. El descuento porcentual no supera 100 y el descuento de monto de un ítem no supera **su precio unitario**. Descuento general de monto no supera la suma después de los descuentos de ítem.
- Contacto/preparador proceden del usuario que crea o clona; editar conserva el contacto original. No son datos editables del formulario. El prototipo inicial de Bastion se corrige para obtenerlos del Core e ignorar overrides del cliente.
- El listado del servidor ordena fecha descendente y después ID; busca número, referencia, cliente, estado y descripción. No filtra por creador en esta copia. La paridad de permisos con una sesión real aún requiere validación.

## Cálculo que persiste el backend

Referencia: `validatedQuote`, líneas 388–561. `R` representa redondeo a dos decimales, mitad alejándose de cero. Bastion usa enteros escalados y operaciones comprobadas; no replica errores binarios de `float`.

1. Normalizar cantidad, precio y descuentos con `R`. Subtotal general bruto: suma sin redondear de cantidad × precio.
2. Subtotal de línea: `R(cantidad × precio)`. Descuento **unitario**: monto o `R(precio × porcentaje / 100)`. Descuento del ítem: `R(cantidad × descuento unitario)`. Base tras ítem: `R(cantidad × (precio − descuento unitario))`.
3. Descuento general: monto o `R(suma bases tras ítem × porcentaje / 100)`.
4. Prorrateo por línea: `R(descuento general × base de línea / suma de bases)`. No existe redistribución del residuo de centavos. Descuento de línea: mínimo entre su subtotal y `R(prorrateo + descuento de ítem)`.
5. Base imponible: máximo entre cero y `R(base tras ítem − prorrateo)`. Agrupar por exento/ISV 15/18; impuesto de línea `R(base × tasa)`.
6. Impuestos del documento: `R(suma de bases de cada tasa × tasa)`, **no la suma de impuestos redondeados por línea**. Total general: `R(subtotal bruto sin redondear − suma descuentos de línea + ISV15 + ISV18)`.
7. `items[].total` conserva la base tras descuento de ítem, antes de descuento general e impuestos. Se preserva este significado en la respuesta; no se etiqueta como importe final con impuesto.
8. La presencia de cualquiera de tres referencias de exoneración hace que las bases se agrupen como exoneradas y las líneas calculadas usen `none`. El frontend activo fuerza esas referencias a vacío y `hasExoneration()` retorna falso: **no se añaden controles de exoneración** a la UI sin acuerdo. El servicio de dominio reconoce la regla observada para cotejo, sin convertirla en una validación fiscal externa.

## Diferencias y límites pendientes

La vista previa Angular calcula sin los redondeos intermedios del backend y suma impuestos por línea. Bastion hace la vista previa con el mismo caso de uso Rust que guarda, para mostrar lo que se persistirá. Es una corrección técnica explícita de consistencia, no una fórmula tributaria nueva.

El prorrateo legado puede distribuir más o menos centavos que el descuento general solicitado; se conserva y se prueba, sin introducir reparto de residuos. El total bruto sin redondear y el redondeo de descuentos por línea también pueden producir resultados límite no intuitivos con cantidades fraccionarias. Deben reconciliarse con ejemplos anonimizados antes de producción; no afirmar equivalencia contable por esta revisión estática.

Pendientes de implementación/paridad: catálogo Clientes, PDF/Typst, adjuntos privados, correo/programación, clonación, eliminación confirmada, conversiones y acciones masivas. Pendientes de validación: sesión real, esquema SQL Legacy, ejemplos contables y continuidad del contador al migrar. Los settings propios siguen sin definirse.
