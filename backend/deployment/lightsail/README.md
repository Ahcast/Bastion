# Lightsail — runtime del backend

Ejecuta backend Go, API, MCP, crawler Python y workers/scheduler como servicios separados. Capacidad inicial propuesta: 2 GB RAM, 2 vCPU, 60 GB SSD; aprovisionamiento por confirmar. Sin MySQL local, OCR o inferencia.

Base de datos remota en Bluehost; archivos S3; SMTP Bluehost. IP estática, HTTPS y conexión DB cifrada con verificación o túnel SSH validado. Medir memoria, CPU, disco y duración del crawler; limitar solapamientos.

Compilar Go fuera de la instancia cuando sea posible, desplegar binarios y configurar systemd. El crawler mantiene su instalación Python y timers. Ver [Core](../../../docs/architecture/core.md).
