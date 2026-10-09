# Instalación paralela — no ejecutar sobre el directorio legacy

1. Respaldar /home/ubuntu/crawler, su crontab y configuración. Confirmar Python 3.12+
   instalado (Ubuntu 22.04 original puede tener Python 3.10). No sustituir Python del sistema.
2. Crear usuario técnico y directorios:
```bash
sudo useradd --system --home /var/lib/bastion-crawler --shell /usr/sbin/nologin bastion-crawler
sudo install -d -o bastion-crawler -g bastion-crawler -m 700 /var/lib/bastion-crawler
sudo mkdir -p /opt/bastion-crawler
```
3. Copiar paquete a /opt/bastion-crawler, crear .venv con Python 3.12 e instalar
   requirements.lock. Mantener código propiedad del administrador.
4. Copiar .env.example a /etc/bastion-crawler.env; chmod 600, propietario root.
   systemd lee EnvironmentFile antes de bajar privilegios. La CLI manual debe
   recibir las variables por un mecanismo autorizado (el usuario técnico no leerá ese archivo).
5. Probar en directorio temporal, un límite bajo de páginas y entregas desactivadas.
   Revisar HTML reales, campos y consumo; una prueba limitada termina parcial por diseño.
6. Configurar inbox/API de staging y AWS. Verificar ACK, hashes y permisos S3.
7. Sólo tras comparar resultados y validar el importador, desactivar las entradas
   legacy de cron (crawler, uploader y healthchecks); conservar copia exacta.
8. Instalar unidades:
```bash
sudo cp deploy/*.service deploy/*.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now bastion-crawler.timer bastion-retry.timer
systemctl list-timers 'bastion-*'
```
Los timers usan explícitamente America/Tegucigalpa (01:00 y 19:00), independiente de
la zona horaria del servidor. Cambiar horarios editando timer, no Python.
Se mantiene un lock global para run/retry/upload. Si otra tarea posee el lock,
la nueva termina sin iniciar extracción; el timer siguiente vuelve a intentarlo.

Rollback: detener/deshabilitar ambos timers, esperar o detener servicios, restaurar
crontab legacy. Los directorios y CSV originales no fueron modificados por este paquete.
Conservar data nuevo para diagnóstico. No limpiar pendientes antes de respaldarlos.

IAM para S3: permitir s3:PutObject y s3:GetObject sólo en
arn:aws:s3:::castilloit-honducompras-crawler/raw/* . El uploader no borra ni lista
buckets. Configurar Block Public Access y versioning mediante AWS Console.
El paquete no crea buckets ni usuarios AWS. No presume un rol EC2 conectado a Lightsail.

Logs: journalctl -u bastion-crawler -u bastion-retry. Configurar límite journald y
monitor externo que consulte status y espacio libre. El límite MemoryMax=600M es
un techo de protección, no una medida de consumo observada.


