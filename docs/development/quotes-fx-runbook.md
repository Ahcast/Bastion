# Ejecutar registro FX de Cotizaciones

Esta unidad administra **observaciones históricas USD/HNL**. No convierte cotizaciones, no selecciona una tasa por defecto y no aplica TCR/compra/venta automáticamente.

Contrato normativo: [quotes-fx.md](../contracts/quotes-fx.md). Contrato REST: [quotes-fx-openapi.json](../contracts/quotes-fx-openapi.json). La política comercial futura sigue separada en [quotes-fx-proposal.md](quotes-fx-proposal.md), que no es ejecutable ni normativa.

## Requisitos

1. Ejecutar migraciones hasta `0014_quote_fx_observations.sql` sobre MySQL 5.7 aislado de pruebas.
2. Quotes debe estar habilitado.
3. Asignar explícitamente scope `all` para:
   - `cotizaciones.fx.view` para consultar;
   - `cotizaciones.fx.manage` para registrar/corregir.
4. Registrar/corregir exige actor humano y reautenticación reciente.
5. Angular expone la pantalla en `/quotes/fx`; no se agrega otro módulo ni entrada de menú.

## REST

- `GET /api/v1/quote-fx/observations`
- `GET /api/v1/quote-fx/observations/{id}`
- `POST /api/v1/quote-fx/observations`

El namespace `quote-fx` se mantiene separado de `/quotes/{id}` para evitar ambigüedad del router.

El POST es DTO completo y estricto. Campos opcionales se envían como cadena vacía, no se omiten. Requiere `Idempotency-Key`. `rate` viaja como cadena decimal exacta; no usar `Number`/float para la tasa.

## Flujo manual de prueba

1. Iniciar API/Angular con la base de prueba migrada.
2. Iniciar sesión con un usuario humano que posea ambos permisos FX con scope `all`.
3. Abrir `/quotes/fx`.
4. Confirmar identidad desde el componente de reautenticación.
5. Registrar una observación sintética, por ejemplo tipo `tcr`, fuente `bch`, referencia sintética, vigencia explícita de Honduras y una tasa con hasta 12 decimales.
6. Verificar que la fila aparece sin modificar ninguna cotización.
7. Usar “Preparar corrección” para crear una fila nueva que referencia la anterior; la tasa original debe permanecer intacta y marcarse como reemplazada.
8. No usar datos productivos ni afirmar que una referencia sintética fue obtenida del BCH.

## Pruebas obligatorias

Backend:

```bash
cd backend
go vet ./...
gofmt -l cmd internal database tests
go test -race -count=1 ./...
make build
```

Con `BASTION_TEST_MYSQL_DSN` apuntando exclusivamente a `bastion_test` en MySQL 5.7. La integración cubre scope global, precisión, UTC, replay, correcciones append-only y rollback de auditoría.

Frontend:

```bash
cd frontend
npm ci
npm run build
npm test -- --watch=false
npx playwright test
```

Playwright debe ejecutarse contra la composición sintética del repositorio. El recorrido FX verifica reautenticación, registro exacto y el aviso explícito de que la observación **no convierte cotizaciones**.

Documentación:

```bash
python3 scripts/check_repository.py
```

El guard combina `quotes-openapi.json` y `quotes-fx-openapi.json`; su unión debe coincidir exactamente con las rutas REST registradas del módulo Quotes y no puede documentar una ruta dos veces.

## Lo que esta unidad no valida

- conectividad o autenticidad real del BCH;
- captura automática del TCR;
- política TCR/compra/venta para una operación;
- fallback de vigencia para fines de semana/días sin publicación;
- redondeo de una futura conversión;
- cambio de moneda de una cotización;
- representación FX en PDF/exportación;
- conversiones Cotización→OC/Factura.

Cualquiera de esas capacidades requiere contrato normativo adicional antes de código.
