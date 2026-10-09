# Una sola rama

**Solo existe y se usa main**, tanto en el remoto como en el checkout de trabajo. No crear otras ramas, forks ni ramas temporales para PR; no branches release/develop/staging. Las versiones son commits. Prohibido force-push y reescritura del historial. Correcciones con nuevos commits o revert.

Antes de trabajar: comprobar refs remotas, actualizar main, inspeccionar cambios existentes. Un escritor por checkout. Antes de push: validar, comprobar que main remoto no cambió, resolver conflictos y publicar fast-forward. No borrar trabajo ajeno para conseguir push.

La política es obligatoria para humanos, agentes y bots. No configurar Dependabot/Renovate o un agente para proponer ramas automáticamente. CI en push main; no exigir PR como requisito incompatible con esta política.

## Comprobación automática

Workflow single-branch corre al publicar en main y al crear/borrar refs. Lista todas las ramas, exige main como rama por defecto y elimina únicamente ramas que sean ancestros de main, con SHA verificado. Una rama divergente nueva produce fallo visible y requiere preservar/reconciliar su trabajo antes de eliminarla; no descarta código ajeno automáticamente.

Este control es reactivo: puede existir una rama durante el intervalo entre creación y limpieza. Para prohibir su creación en todo momento se requiere ruleset GitHub activo para todas las ramas salvo main, con Restrict creations y sin bypass de bots, más reglas de main compatibles con commits directos. Su disponibilidad depende del plan/permisos y no se acredita mediante un Markdown o un check CI.

La limpieza actual conserva los SHA de las ramas antiguas como padres del commit consolidado; no crea ramas de backup ni tags adicionales. El historial queda accesible desde main. Verificar después que la API lista exactamente main.

## Resultado verificado

Las tres ramas anteriores se eliminaron y solo main permanece. CI de limpieza pasó. GitHub devolvió 403: rulesets requieren GitHub Pro o repositorio público; mantener la privacidad actual. No afirmar que creación esté bloqueada preventivamente con el plan actual.
