# Crawler HonduCompras existente

Implementación Python en bastion_crawler, con SQLite técnico, exportación y entrega recuperable. Se ejecuta en Lightsail junto al backend, en servicio separado. No convertir a Go ni aumentar agresividad por esta reorganización.

[Operaciones](OPERATIONS.md), [instalación](deploy/INSTALL.md) y [contrato API/ACK](docs/IMPORT_CONTRACT.md) son fuentes únicas. Destino objetivo: API Go en Lightsail; MySQL permanece en Bluehost. El target interno bluehost y FTPS se conservan para compatibilidad histórica; FTPS deshabilitado en objetivo inicial.

## Desarrollo y pruebas offline

```bash
cd backend/crawler
python -m venv .venv
.venv/bin/python -m pip install -r requirements.lock
.venv/bin/python -m unittest discover -s tests -v
.venv/bin/python -m bastion_crawler --help
```

La ayuda del código define CLI real; no documentar comandos conceptuales como disponibles. Tests offline no acreditan extracción en vivo, ACK receptor implementado ni integración S3/MySQL. Credenciales en entorno privado, nunca Git. No publicar estado SQLite, CSV reales, logs o documentos comerciales.
