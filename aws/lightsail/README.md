# Lightsail

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 5. Infraestructura actual

## 5.1 Bluehost

Castillo IT tiene **Bluehost Choice Plus Hosting**.

Actualmente Bluehost aloja:

- aplicación web;
- Laravel;
- Angular SPA;
- MySQL;
- correo electrónico de Castillo IT;
- archivos asociados a la aplicación.

Bluehost continuará siendo inicialmente el **Cardona/Bastion Nucleo**.

## 5.2 AWS Lightsail

Instancia actual:

- **1 GB RAM**
- **2 vCPU**
- **40 GB SSD**

Responsabilidades previstas:

- crawler HonduCompras;
- Python;
- cron/scheduler;
- retries;
- checkpoints;
- estado técnico local;
- generación de CSV/JSON;
- subida de raw a S3;
- backup worker;
- sincronizaciones técnicas;
- logs.

El Lightsail actual **no se considera adecuado para un RAG/vector DB completo** por la RAM disponible.

## 5.3 Amazon S3

Bucket existente:

`castilloit-honducompras-crawler`

Región:

`us-east-1` — Norte de Virginia.

Usos:

- raw crawler;
- snapshots;
- CSV;
- JSON;
- manifests;
- logs;
- documentos descargados;
- reconstrucción/replay.

Bucket propuesto adicional:

`castilloit-backups`

Usos:

- backups de MySQL;
- backups de Bluehost;
- uploads;
- documentos;
- aplicación;
- manifests de backups.

En el futuro puede crearse:

`castilloit-documents`

para separar documentación operativa de backups y raw del crawler.

## 5.4 GitHub

GitHub será la fuente oficial del código.

Repositorios propuestos:

```text
CastilloIT/
├── bastion
│   ├── servidor
│   └── interfaz
├── honducompras-crawler
└── infrastructure
```

Alternativa inicial simplificada:

```text
CastilloIT/
├── bastion
└── honducompras-crawler
```

---


# 7. Responsabilidad por ubicación

## 7.1 Bluehost — Bastion Nucleo

Ejecuta:

- Angular SPA;
- Laravel;
- REST API;
- autenticación;
- RBAC;
- workflow engine;
- approval engine;
- audit log;
- importador de procesos;
- lógica de negocio;
- integración con MySQL;
- endpoints para agentes;
- endpoints internos;
- generación de documentos cuando sea razonable;
- jobs según capacidad del hosting.

Almacena en MySQL:

- usuarios;
- roles;
- permisos;
- clientes;
- proveedores;
- procesos;
- versiones de procesos;
- items;
- cotizaciones;
- órdenes de compra;
- facturas;
- gastos;
- RFQ;
- respuestas de RFQ;
- precios;
- tareas;
- workflows;
- aprobaciones;
- auditoría;
- estado IA;
- referencias a documentos.

## 7.2 AWS Lightsail

Ejecuta:

- crawler HonduCompras;
- scheduler;
- retries;
- checkpoints;
- estado técnico del crawler;
- generación raw;
- subida a S3;
- entrega de resultados a Bluehost;
- backup worker;
- sincronización de backups a S3.

No debe convertirse en fuente maestra de negocio.

## 7.3 Amazon S3

Almacena:

- raw del crawler;
- CSV;
- JSON;
- run manifests;
- error manifests;
- documentos históricos;
- PDFs;
- datasheets;
- snapshots;
- backups MySQL;
- backups de archivos;
- documentos generados;
- archivos de colaboración.

No reemplaza MySQL.

## 7.4 GitHub

Almacena:

- código;
- migrations;
- tests;
- documentación;
- configuración no secreta;
- scripts;
- workflows CI/CD;
- infraestructura declarativa.

Nunca almacenar:

- `.env`;
- contraseñas;
- API keys;
- AWS keys;
- dumps SQL;
- CSV empresariales;
- documentos sensibles.

---

