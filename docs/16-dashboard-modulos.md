# Inicio con datos de los módulos operativos

Decisión confirmada el 2026-10-23: los módulos operativos alimentan el Inicio. Se mantienen cinco tarjetas. A partir del 2026-10-26, las cinco áreas —Procesos, Cotizaciones, Órdenes, Facturas y Gastos— solicitan un resumen propio. Los datos se consultan solo con el permiso de lectura de cada módulo.

## Contrato y límites

- Mes actual (predeterminado), Año actual y Todo, con botón Actualizar. Calendario de Honduras (`America/Tegucigalpa`, UTC−6); inicio inclusivo y fin exclusivo.
- `GET /api/v1/procesos/dashboard?period=month|year|all`: requiere módulo activo y `procesos/consulta/ver`. Cuenta únicamente recepciones posteriores al instante del servidor dentro del período elegido. Indicadores: vigentes, marcados de interés, entidades distintas y recepciones hoy. Distribución Hoy/Mañana/Después, gráfico por recepción y hasta ocho próximas recepciones. No inventa estados del proceso ni devuelve notas o JSON fuente.
- `GET /api/v1/cotizaciones/dashboard?period=month|year|all`: requiere módulo activo y `cotizaciones/documentos/ver`. Usa `quote_date`, estado y total calculado guardados; cuenta cotizaciones y suma sus importes en HNL, incluidos todos los estados. Distingue cotizaciones antiguas sin total calculado y no les atribuye un precio. Distribución por estado, gráfico de montos y hasta ocho documentos recientes. No devuelve ítems, notas, correos ni documentos privados.
- `GET /api/v1/ordenes-compra/dashboard?period=month|year|all`: requiere módulo activo y `ordenes_compra/documentos/ver`. Agrupa por fecha de la orden, presenta cantidad/total en HNL, distribución por estados observados y hasta ocho órdenes recientes. No devuelve productos, comentarios ni datos de tracking.
- `GET /api/v1/facturas/dashboard?period=month|year|all`: requiere módulo activo y `facturas/documentos/ver`. En el alcance actual los registros son borradores; suma únicamente totales calculados, separa los registros sin total, presenta distribución por estado y hasta ocho borradores recientes con saldo. No presenta pagos recibidos como cobrados ni supone facturas emitidas.
- `GET /api/v1/gastos/dashboard?period=month|year|all`: requiere módulo activo y `gastos/registros/ver`. Cuenta registros no eliminados y muestra serie por cantidad, distribución entre los proveedores con más registros y hasta ocho gastos recientes. Los totales se agrupan por moneda; nunca suma monedas diferentes.
- Cada respuesta incluye ventana, zona horaria, instante de cálculo, indicadores, serie, distribución y registros. Los rangos Mes/Año completan días/meses sin actividad con cero. Todo agrupa por mes y limita el gráfico a los primeros 120 meses con actividad, con aviso explícito; totales y distribución cubren el período completo.
- Los importes se suman en SQL decimal y viajan como cadenas; el navegador conserva centavos mediante enteros de precisión arbitraria. El monto cotizado no representa cobros ni saldo financiero.
- El Core comparte únicamente el contrato de período/calendario; cada módulo calcula sus propias cifras dentro de su transacción y autorización. No se añade un permiso que omita el control del módulo ni acceso SQL desde MCP.
- El frontend solicita por separado únicamente los resúmenes autorizados. Un error en un módulo permite consultar el otro. Distingue carga, cero, falta de permiso, integración pendiente y error con reintento. Los cambios de período cancelan las solicitudes anteriores.
- Inicio actualiza solo los módulos habilitados y visibles para el usuario; la cifra de la tarjeta y sus detalles proceden del mismo resumen del módulo. Un error parcial no oculta el resto del panel.
- Seleccionar una tarjeta actualiza indicadores, gráficos y tabla. Los enlaces al módulo respetan la navegación existente. En celular, tarjetas, gráfico y tabla permiten desplazamiento horizontal interno sin ampliar toda la página; controles y pestañas conservan etiquetas y navegación por teclado.

## Verificación

1. Con permisos de lectura en cada área, entrar a Inicio: contrastar los conteos, fechas, estados, saldos e importes con los listados y reportes de los cinco módulos. Confirmar que Gastos no sume monedas distintas.
2. Elegir Año/Todo, y Actualizar. Una cotización fuera del rango no debe sumarse. Un proceso vencido no debe aparecer ni contarse. Las fechas usan Honduras aunque el navegador esté en otra zona.
3. Probar roles con acceso a distintos subconjuntos: no debe solicitar ni mostrar información de los módulos sin permiso. Desactivar un módulo y consultar su ruta debe bloquear también al administrador.
4. Probar un período vacío y un error de API: cero y error deben diferenciarse; el otro resumen debe seguir disponible. Revisar la pantalla a 320/390/768 px y escritorio.
5. Pruebas automatizadas: calendario y año bisiesto; MySQL 5.7 con permisos, desactivación, exclusión de vencidos, rango y suma decimal; frontend con permisos, selección, actualización, cancelación, errores parciales, texto seguro e importes grandes.

No requiere migraciones ni modifica registros de negocio. Al añadir una nueva fuente se publica backend AWS y frontend estático Bluehost desde artifacts del mismo commit verificado.
## Publicación verificada · 2026-10-23

- Primera publicación del dashboard en AWS y Bluehost: `e55b430ab8134771f1c734cd5ca8332450f06d98`, publicado directamente en `main`, sin ramas adicionales.
- [GitHub Actions 39940822106](https://github.com/Ahcast/Bastion/actions/runs/39940822106): formato, compilación, 60 pruebas Rust (incluidas bases MySQL 5.7 aisladas), 63 pruebas frontend, build y auditoría de dependencias de producción aprobados.
- Artifacts de backend y frontend verificados contra sus SHA-256, manifiestos y commit. Treinta archivos públicos contrastados por HTTPS con el manifiesto original; `assets/.htaccess` se comprueba en el despliegue y permanece protegido.
- Backend con salud y disponibilidad en HTTP 200; ambas rutas de dashboard devuelven HTTP 401 sin sesión. Se conservan la configuración del proxy, las reglas de Bluehost y los respaldos privados para reversión. No requiere migraciones ni altera datos de negocio.
- Diseño revisado con el paquete de CI a 320, 390 y 768 px y escritorio, usando datos sintéticos. El acceso público carga el formulario de inicio de sesión sin errores de consola. La revisión visual no utilizó ni sustituyó una sesión real del usuario en producción.
