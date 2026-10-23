# Inicio con datos de Procesos y Cotizaciones

Decisión confirmada el 2026-10-23: los módulos existentes alimentan el Inicio. Se mantienen cinco tarjetas; Procesos y Cotizaciones aportan resúmenes reales. Las otras áreas indican que su integración al Inicio está pendiente.

## Contrato y límites

- Mes actual (predeterminado), Año actual y Todo, con botón Actualizar. Calendario de Honduras (`America/Tegucigalpa`, UTC−6); inicio inclusivo y fin exclusivo.
- `GET /api/v1/procesos/dashboard?period=month|year|all`: requiere módulo activo y `procesos/consulta/ver`. Cuenta únicamente recepciones posteriores al instante del servidor dentro del período elegido. Indicadores: vigentes, marcados de interés, entidades distintas y recepciones hoy. Distribución Hoy/Mañana/Después, gráfico por recepción y hasta ocho próximas recepciones. No inventa estados del proceso ni devuelve notas o JSON fuente.
- `GET /api/v1/cotizaciones/dashboard?period=month|year|all`: requiere módulo activo y `cotizaciones/documentos/ver`. Usa `quote_date`, estado y total calculado guardados; cuenta cotizaciones y suma sus importes en HNL, incluidos todos los estados. Distingue cotizaciones antiguas sin total calculado y no les atribuye un precio. Distribución por estado, gráfico de montos y hasta ocho documentos recientes. No devuelve ítems, notas, correos ni documentos privados.
- Cada respuesta incluye ventana, zona horaria, instante de cálculo, indicadores, serie, distribución y registros. Los rangos Mes/Año completan días/meses sin actividad con cero. Todo agrupa por mes y limita el gráfico a los primeros 120 meses con actividad, con aviso explícito; totales y distribución cubren el período completo.
- Los importes se suman en SQL decimal y viajan como cadenas; el navegador conserva centavos mediante enteros de precisión arbitraria. El monto cotizado no representa cobros ni saldo financiero.
- El Core comparte únicamente el contrato de período/calendario; cada módulo calcula sus propias cifras dentro de su transacción y autorización. No se añade un permiso que omita el control del módulo ni acceso SQL desde MCP.
- El frontend solicita por separado únicamente los resúmenes autorizados. Un error en un módulo permite consultar el otro. Distingue carga, cero, falta de permiso, integración pendiente y error con reintento. Los cambios de período cancelan las solicitudes anteriores.
- Seleccionar una tarjeta actualiza indicadores, gráficos y tabla. Los enlaces al módulo respetan la navegación existente. En celular, tarjetas, gráfico y tabla permiten desplazamiento horizontal interno sin ampliar toda la página; controles y pestañas conservan etiquetas y navegación por teclado.

## Verificación

1. Con permisos de lectura en ambos módulos, entrar a Inicio: contrastar las cifras con recepciones vigentes y cotizaciones del mes. Cambiar de tarjeta; comprobar fechas, estados e importes.
2. Elegir Año/Todo, y Actualizar. Una cotización fuera del rango no debe sumarse. Un proceso vencido no debe aparecer ni contarse. Las fechas usan Honduras aunque el navegador esté en otra zona.
3. Probar un rol con acceso a un solo módulo: no debe solicitar ni mostrar información del otro. Desactivar el módulo y consultar su ruta debe bloquear también al administrador.
4. Probar un período vacío y un error de API: cero y error deben diferenciarse; el otro resumen debe seguir disponible. Revisar la pantalla a 320/390/768 px y escritorio.
5. Pruebas automatizadas: calendario y año bisiesto; MySQL 5.7 con permisos, desactivación, exclusión de vencidos, rango y suma decimal; frontend con permisos, selección, actualización, cancelación, errores parciales, texto seguro e importes grandes.

No requiere migraciones ni modifica registros de negocio. Despliegue: backend AWS y frontend estático Bluehost del mismo commit verificado.
