# Visión y alcance

## Visión

Crear una nueva versión de CastilloIT en Rust, organizada como un core con módulos conectables, conservando la experiencia operativa actual y mejorando seguridad, mantenibilidad, API e integración con IA mediante MCP.

## Resultados del producto

1. El usuario puede completar los mismos flujos del sistema actual con los campos, acciones y resultados equivalentes.
2. El core ofrece identidad, permisos, navegación, registro de módulos, auditoría y servicios compartidos.
3. Cada módulo aporta sus pantallas, reglas, datos, permisos, API, herramientas MCP y settings.
4. Al registrar un módulo, el core lo incorpora al área central Ajustes sin requerir un menú de configuración paralelo.
5. API y MCP aplican la misma identidad y autorización que la interfaz.
6. El diseño permite migrar por etapas los módulos y sus datos.
7. El core puede desplegarse solo en los servidores para probarlo antes de incorporar módulos; después los módulos se publican de forma progresiva, uno por uno.
8. El acceso del core usa OTP TOTP en equipos/IP no reconocidos y al cumplirse 15 días, incluso si la sesión continúa abierta; el usuario administra seguridad, dispositivos confiables y sesiones desde sus ajustes.

## Alcance funcional inicial

El catálogo sale del informe de la aplicación: Clientes; Proveedores y destinos; Órdenes de compra; Gastos; Cotizaciones; Facturas; Reportes; Procesos; Colaboración y archivos. El core incluye Acceso, Dashboard, Perfil, Usuarios, Roles/Permisos, Personas y configuración de empresa/correo observados en la aplicación actual.

## Ampliación de arquitectura: Licitaciones

El 2026-10-27 se incorpora al diseño el módulo nuevo [Licitaciones](modulos/licitaciones.md), a solicitud del usuario y tras revisar su compatibilidad con el código vigente. Gestiona expedientes con bloques legal, económico y técnico, variables aisladas, copias de recursos de Colaboración y un PDF final versionado. Se prevé crear borradores vinculados en Facturas mediante un contrato posterior; no implica emisión fiscal.

Es una ampliación propuesta del producto, no paridad Legacy ni una extensión de Procesos/Scout. Sus campos, reglas y contratos pendientes se detallan en la ficha. La incorporación es documental: no lo incluye en el runtime, no ejecuta migraciones ni lo despliega o activa.

## Alcance técnico acordado

- Backend nuevo en Rust.
- Frontend servido desde Bluehost.
- API y MCP del core ejecutados en AWS.
- PDFs y adjuntos privados guardados en almacenamiento privado de objetos en AWS; no caducan mientras exista el registro asociado y se eliminan con ese registro mediante permiso, confirmación y auditoría. El servicio concreto y los respaldos se definen en el diseño técnico.
- MySQL alojado en Bluehost para la primera arquitectura acordada.
- Monolito modular como punto de partida, con límites de módulos claros. La primera publicación en servidores contiene solo el core; luego se incorpora un módulo por release y se valida antes de continuar.
- La publicación progresiva es un requisito. En la primera etapa, cada release conserva el backend modular común; no implica que cada módulo sea un microservicio independiente ni que se cargue código dinámico.

## Principio de paridad

El informe actual es la base de comportamiento. Una mejora de diseño o seguridad puede cambiar la implementación interna, pero cualquier cambio del flujo visible, campos o permisos debe documentarse como decisión explícita.

## Dirección visual

La exportación HTML del panel que compartió el usuario es la referencia visual preferida, especialmente por sus colores. Se extrajeron sus tokens y componentes; ver `12-direccion-visual.md`. Validar contraste y legibilidad al implementarlos.

## Fuera de alcance hasta acordarlo

- Añadir módulos de negocio que no aparezcan en el informe sin una decisión explícita; Licitaciones tiene incorporación documental y decisiones pendientes según su ficha.
- Cambiar reglas tributarias/comerciales por inferencia.
- Definir proveedor o framework específico de UI, runtime AWS o ORM Rust.
- Migrar datos productivos o cambiar el host MySQL sin una decisión posterior.

## Pendientes de verificación y detalle por módulo

- Verificar visualmente con sesión autenticada los formularios, estados y mensajes del Legacy. Las pantallas conocidas se documentaron desde el código y el informe estático; no afirmar equivalencia visual hasta revisar una sesión activa.
- La matriz de roles del Legacy no está disponible. Para el Core nuevo queda definido un rol inicial de Administrador del sistema; roles personalizados se crean sin permisos y reciben permisos según necesidad de los módulos.
- Obtener esquema MySQL sin datos privados.
- Acordar settings específicos por módulo que no existan hoy, en cada ficha de módulo.
