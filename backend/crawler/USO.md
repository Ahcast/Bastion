# Ejecutar el crawler en tu máquina

Esta guía corre una extracción completa de HonduCompras y guarda los resultados **solo en tu máquina**. El crawler ya está versionado en `main`, dentro de `backend/crawler`. Requiere Python 3.12 o posterior, Git y acceso a Internet. La primera corrida puede durar horas porque consulta el sitio de forma secuencial y espera entre peticiones.

## Preparación

En una terminal de Linux/macOS (o WSL en Windows):

```bash
git clone https://github.com/Ahcast/Bastion.git
cd Bastion/backend/crawler
python3.12 -m venv .venv
.venv/bin/python -m pip install -r requirements.lock
cp .env.example .env
```

Si ya tienes el repositorio, desde su raíz ejecuta `git pull --ff-only origin main` y entra a `backend/crawler`. El repositorio puede pedir autenticación de GitHub si es privado. En un sistema donde `python3` ya sea 3.12+, puedes usar `python3` para crear el entorno.

Edita `.env` para guardar los datos en el directorio actual y dejar las entregas desactivadas:

```dotenv
CRAWLER_DATA=./data
S3_ENABLED=false
BLUEHOST_MODE=disabled
MAX_PAGES=500
MAX_RUN_SECONDS=14400
REQUEST_DELAY=3
```

`CRAWLER_DATA=./data` es importante: el ejemplo del servidor usa `/var/lib/bastion-crawler`, que normalmente requiere permisos especiales. No coloques credenciales en Git.

## Ejecución completa

```bash
.venv/bin/python -m bastion_crawler run
```

Déjalo terminar. **No uses `--dry-run` para esta prueba:** ese modo descarta sus archivos al salir. `MAX_PAGES` (500) y `MAX_RUN_SECONDS` (4 horas) son topes de protección. Si se alcanzan, la corrida queda parcial; no son una promesa de recorrer todo HonduCompras. Para una corrida más larga, aumenta ambos en `.env` con prudencia. Mantén `REQUEST_DELAY=3` o más para respetar el ritmo conservador. Solo una instancia debe ejecutarse a la vez.

La salida del comando vale 0 si terminó la extracción, 2 si fue parcial y 1 si falló. La entrega a S3/API está desactivada en esta prueba y no afecta ese estado.

## Revisar resultados y recuperar pendientes

```bash
.venv/bin/python -m bastion_crawler status
ls -lt data/batches
```

En `data/batches/<BATCH_ID>/` encontrarás `processes.csv`, `processes.json`, `run.json`, `errors.json` y `manifest.json`. `processes.csv` contiene **lo observado en ese lote**, aunque la extracción haya sido parcial; no representa la base histórica completa. Los archivos RAW quedan en `data/runs/<RUN_ID>/raw/` y el estado técnico en `data/state.sqlite3`. Una nueva ejecución crea otro lote; no borra el anterior.

Si hay unidades pendientes, espera el plazo de reintento indicado en el estado (normalmente al menos 15 minutos) y ejecuta:

```bash
.venv/bin/python -m bastion_crawler retry
.venv/bin/python -m bastion_crawler status
```

Para comprobar la integridad de un lote concreto:

```bash
.venv/bin/python -m bastion_crawler validate data/batches/<BATCH_ID>
```

No ejecutes `upload` ni `replay` salvo que hayas configurado y validado un destino. La consolidación de NEW/UPDATED, historial e idempotencia de negocio corresponde al receptor Go/MySQL, que aún no está implementado. Una corrida local comprueba extracción y artefactos, no una importación a producción.

## Pruebas offline

```bash
.venv/bin/python -m unittest discover -s tests -v
```

Consulta [OPERATIONS.md](OPERATIONS.md) para detalles de reintentos y límites, [instalación Lightsail](deploy/INSTALL.md) para el servicio programado y [contrato de importación](docs/IMPORT_CONTRACT.md) para la futura API.
