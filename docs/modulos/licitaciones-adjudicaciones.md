# Adjudicaciones y borrador de factura

El usuario confirma seleccionar ítems y cantidades adjudicadas, elegir un cliente del catálogo y revisar el borrador. Los precios e impuestos se capturan durante esa revisión.

## Contrato del flujo

La nueva sección Adjudicaciones permite registrar cantidades positivas de ítems de una oferta Enviada. Cada registro conserva una instantánea del expediente, lote, posición del ítem y descripción. Las adjudicaciones son independientes del contenido documental presentado. La suma adjudicada de cada ítem se limita a la cantidad ofertada mediante un bloqueo transaccional del expediente. La posición del ítem comienza en uno. Las cantidades conservan hasta doce decimales, como la oferta.

La revisión de factura reutiliza el editor de Facturas. Exige cliente de catálogo, cantidades iguales a las adjudicadas y precios e impuestos introducidos por el usuario. Facturas valida sus límites y calcula el documento. Si su representación decimal no conserva una cantidad adjudicada, la creación se rechaza y se presenta el motivo. El máximo técnico de selección es de 200 ítems por adjudicación, coincidente con el límite vigente de Facturas.

Una adjudicación genera como máximo un borrador. El coordinador document-flows comprueba lectura y edición en ambos dominios, guarda el recibo idempotente y la relación única en la misma transacción que la factura. El borrador conserva origen de Licitaciones. Este flujo permite registrar adjudicaciones adicionales sobre cantidades restantes.

La creación del vínculo se limita a borradores a crédito sin pago inicial. Los flujos ordinarios de Facturas gobiernan emisión, pagos y corrección posteriores. No se añaden estados al expediente presentado. Las reglas de modificación o anulación de una adjudicación requieren un contrato posterior.

## API

- GET y POST `/api/v1/licitaciones/{id}/adjudicaciones` consultan y registran adjudicaciones.
- GET `/api/v1/licitaciones/{id}/adjudicaciones/{award}` consulta su instantánea.
- POST `/api/v1/licitaciones/{id}/adjudicaciones/{award}/factura` guarda el borrador revisado.

POST requiere sesión, origen, CSRF, clave idempotente y If-Match. El registro usa la versión del expediente y la factura usa la versión de adjudicación. Se conservan cuotas y auditoría. El contrato detallado está en licitaciones-adjudicaciones.openapi.yaml.

## Validación requerida

La integración debe verificar permisos, desactivación, selección parcial, cantidades acumuladas, concurrencia, reintentos, cliente de catálogo, precisión decimal, unicidad y conservación de datos anteriores. Las migraciones se aplican después de CI y de un respaldo recuperable. La implementación y la disponibilidad en producción se registran por separado.
