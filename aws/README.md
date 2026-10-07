# AWS

Responsabilidades aprobadas en el documento maestro:

- [Lightsail](lightsail/README.md): crawler HonduCompras, Python, scheduler, retries, checkpoints, estado técnico, entrega a Bluehost y backup worker.
- [S3](s3/README.md): raw, snapshots, documentos, histórico y backups.
- [Seguridad](security/README.md)
- [Despliegue](deployment/README.md)

Instancia documentada: 1 GB RAM, 2 vCPU, 40 GB SSD. No es adecuada para un RAG/vector DB completo. El RAG inicial recupera antecedentes mediante Laravel/MySQL y documentos S3; no instala vector DB en Lightsail.
No se asignan workers OCR, AI o compilación pesada a AWS sin una decisión aprobada.
