# S3 y almacenamiento

Fuente: [Documento maestro vigente](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 5. Infraestructura actual

## 5.1 Bluehost
Plan Choice Plus: destino de Angular, Laravel, MySQL y correo. La nueva aplicación Bastion aún no se acredita desplegada.
- Dominio: `https://bastion.castilloit.net`.
- PHP seleccionable 8.1–8.5; objetivo 8.5, CLI/extensiones por verificar.
- SSH y FTP disponibles; usar SSH/SFTP para despliegue.
- Cron mínimo de un minuto.
- Document root: `~/public_html/website_091cdca8`; exponer solo activos públicos.
- Servidor reportado: **Percona Server for MySQL 5.7.44-48**. Compatibilidad de paquetes/migraciones y soporte de seguridad por validar.

## 5.2 AWS Lightsail
Instancia documentada: 1 GB RAM, 2 vCPU, 40 GB SSD. Ejecuta crawler, retries, checkpoints, entrega y workers de backup. No es la base maestra ni se le asignan modelos locales, OCR pesado o vector DB por defecto.

## 5.3 Amazon S3
Bucket confirmado por el usuario: `castilloit-honducompras-crawler`, región `us-east-1`. Documentos operativos y backups requieren ubicación/aislamiento definidos; nombres como `castilloit-backups` y `castilloit-documents` son propuestas, no buckets cuya creación esté verificada. Archivos privados, versiones y metadatos en MySQL.

## 5.4 GitHub
Repositorio vigente: [Ahcast/Bastion](https://github.com/Ahcast/Bastion). Monorepo con `bluehost/`, `aws/` y `docs/`. GitHub mantiene código, documentación y CI/CD; crawler y backups se ejecutan en Lightsail.

---


# 7. Responsabilidad por ubicación

## 7.1 Bluehost — Bastion Core

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
- estado AI;
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


# 42. Data ownership

| Tipo de dato | Fuente de verdad |
|---|---|
| usuarios | MySQL |
| roles/permisos | MySQL |
| clientes | MySQL |
| proveedores | MySQL |
| procesos actuales | MySQL |
| versiones de procesos | MySQL |
| raw crawler | S3 |
| crawler technical state | SQLite Lightsail |
| cotizaciones | MySQL |
| OC | MySQL |
| facturas | MySQL |
| gastos | MySQL |
| documentos físicos | S3 |
| metadata documentos | MySQL |
| código | GitHub |
| backups | S3 |
| prompts/config AI | GitHub/MySQL según uso |
| Recuperación RAG | MySQL para índices/metadatos y S3 para fuentes/texto; vector search opcional |

---

## Artefactos de IA y RAG

Según [arquitectura IA](../../docs/architecture/ai-integration.md), S3 guarda originales, texto extraído por versión, evidencia, RFQ/adjuntos y fichas. MySQL guarda estado, precios y metadatos. Buckets privados, acceso por documento autorizado, checksum y carga verificada; enlaces temporales no se almacenan como rutas definitivas. S3 Vectors es opcional y distinto de un bucket normal; embeddings/costos por definir. No se crean buckets en esta actualización.
