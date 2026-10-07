# Correo

El correo permanece en Bluehost. Configurar SMTP mediante secretos, verificar TLS y límites de envío. Registrar destinatario, estado y correlación sin almacenar credenciales.
Enviar mediante jobs con deduplicación y revisión cuando corresponda. Ante timeout ambiguo, reconciliar antes de reenviar para evitar duplicados.
Configurar y validar SPF, DKIM y DMARC para el dominio real; no inventar registros.
