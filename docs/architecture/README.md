# Arquitectura y responsabilidades

Fuente: [Documento maestro v0.5](../../docs/Proyecto_Bastion_Documento_Maestro.md). Alcance conceptual; implementación y validación técnica pendientes.

# 4. Principios de arquitectura

Las reglas base del proyecto son:

1. **MySQL es la fuente de verdad de datos estructurados y transaccionales.**
2. **S3 es almacenamiento histórico, documental, raw y de backups.**
3. **GitHub es la fuente de verdad del código.**
4. **Laravel controla la lógica empresarial.**
5. **Angular es la interfaz principal para usuarios.**
6. **Lightsail ejecuta extracción y tareas técnicas, no la lógica principal de negocio.**
7. **Los agentes AI nunca acceden directamente a MySQL.**
8. **AI utiliza API/MCP con permisos limitados.**
9. **Las acciones críticas requieren autorización humana.**
10. **Todo cambio importante queda auditado.**
11. **El sistema debe operar aunque ChatGPT o Grok no estén disponibles temporalmente.**
12. **El crawler nunca reemplaza el histórico: observa, acumula, compara, versiona y reporta.**
13. **El sistema debe permitir agregar módulos futuros sin rediseñar el núcleo.**
14. **No se implementarán microservicios prematuramente.**
15. **La arquitectura inicial será un modular monolith.**

---


# 6. Arquitectura general

```text
                                   USUARIOS
                                      │
                                      ▼
                                Angular SPA
                                      │
                                    HTTPS
                                      │
                                      ▼
                            ┌──────────────────┐
                            │ Laravel / Bastion│
                            │      Core        │
                            └────────┬─────────┘
                                     │
                  ┌──────────────────┼──────────────────┐
                  │                  │                  │
                  ▼                  ▼                  ▼
                MySQL               S3          Knowledge/RAG
                  │                                     │
                  │                                     │
                  └────────── Castillo API/MCP ─────────┘
                                     │
                     ┌───────────────┼───────────────┐
                     │               │               │
                     ▼               ▼               ▼
                  ChatGPT         Grok Auto      Grok Assistant


HonduCompras
     │
     ▼
AWS Lightsail
     │
     ├── crawler
     ├── retries
     ├── checkpoints
     ├── raw
     └── backup worker
        │
        ├──────────────► Amazon S3
        │
        └──────────────► Bluehost / Bastion


GitHub
  │
  ├── crawler
  ├── Laravel
  ├── Angular
  └── infrastructure
```

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
| RAG index | servicio RAG futuro |

---


# 43. Conexiones entre componentes

| Origen | Destino | Mecanismo |
|---|---|---|
| Angular | Laravel | HTTPS REST JSON |
| Laravel | MySQL | conexión DB interna |
| Laravel | S3 | AWS SDK/HTTPS |
| Lightsail | HonduCompras | HTTP(S) |
| Lightsail | S3 | boto3/AWS CLI HTTPS |
| Lightsail | Bluehost | actual push a documentar; objetivo HTTPS API |
| Lightsail backup worker | Bluehost | SSH/SFTP/rsync |
| GitHub Actions | Lightsail | SSH |
| GitHub Actions | Bluehost | SSH/SFTP/nativo pendiente |
| ChatGPT | Bastion | MCP/API HTTPS |
| Grok | Bastion | MCP/API HTTPS |
| Office Assistant | Bastion | MCP + OAuth delegated identity |
| Bastion | Email | connector/API/SMTP según diseño final |

---

