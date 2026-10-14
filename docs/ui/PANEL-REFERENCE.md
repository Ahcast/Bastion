# Distribución aprobada: Bastion · Panel

La referencia vigente es el HTML Panel adjunto por el usuario el 14 de octubre de 2026. Sus dos adjuntos son idénticos. El diseño se implementa en Angular y con fuentes autoalojadas. Los datos del prototipo no se incorporan a producción ni al repositorio.

- Navegación horizontal de carbón, logo como Inicio, iconos de módulos, búsqueda de módulos autorizados, alertas, Configuración y cuenta.
- Inicio: cabecera; cinco indicadores seleccionables; fila de pendientes, gráficos y desglose; tabla de trabajo de ancho completo debajo. El indicador activo controla el módulo mostrado. Los controles del período se despliegan bajo el resumen.
- Módulos: cabecera y acción, cuatro indicadores con el primero oscuro, dos secciones de análisis, filtros y tabla. Los conteos por página se identifican explícitamente. Los importes provienen del resumen autorizado y mantienen monedas separadas.
- Filas desplegables: selección gris, junta de carbón a la izquierda y formulario dentro de la tabla. Campos en dos columnas; texto largo e ítems ocupan el ancho disponible. En móvil se apilan y la tabla tiene desplazamiento interno.
- Se conservan todas las acciones comerciales y de seguridad existentes. Configuración y Colaboración conservan su navegación interna y formularios específicos.

No inventar métricas, variaciones semanales o pendientes para reproducir los valores ficticios del ejemplo. Los gráficos de página describen únicamente los registros cargados. La búsqueda superior localiza módulos; no pretende ser búsqueda global de registros.

Validación: build y pruebas Angular; regresión Playwright `concreto-layout.spec.ts` a 1440/390 px y búsqueda con permisos limitados. Las capturas existentes preceden este ajuste hasta que se regeneren con navegador.
