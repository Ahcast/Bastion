# Hosting del frontend

Bluehost Choice Plus aloja Angular compilado, Percona/MySQL y correo. Dominio `bastion.castilloit.net`; document root reportado `~/public_html/website_091cdca8`.

Go, API, MCP, crawler, workers y scheduler se ejecutan en AWS Lightsail. Angular se compila fuera de Bluehost. Nunca incluir credenciales MySQL/S3/SMTP en los activos públicos.
