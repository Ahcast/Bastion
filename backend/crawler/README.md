# Crawler HonduCompras

Crawler Python de Bastion, ubicado en `backend/crawler` y destinado a ejecutarse como servicio separado en Lightsail. Extrae con pausas conservadoras, guarda estado técnico en SQLite y genera lotes recuperables. El receptor Go/MySQL es responsable de consolidar procesos y versiones; todavía no está implementado.

**Para correr una ejecución completa en tu propia máquina:** [guía paso a paso](USO.md).

**Para operación en Lightsail:** [detalles y límites](OPERATIONS.md), [instalación](deploy/INSTALL.md) y [contrato API/ACK](docs/IMPORT_CONTRACT.md).

## Pruebas offline

```bash
cd backend/crawler
python3.12 -m venv .venv
.venv/bin/python -m pip install -r requirements.lock
.venv/bin/python -m unittest discover -s tests -v
.venv/bin/python -m bastion_crawler --help
```

Estas pruebas no acreditan extracción en vivo, ACK del receptor ni integración S3/MySQL. Credenciales y datos reales permanecen fuera de Git.
