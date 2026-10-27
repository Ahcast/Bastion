-- User decision 2026-10-26: ISV collected is excluded from profit.
-- Correct derived values in drafts and idempotency responses, preserving all other fields.
UPDATE facturas_drafts
SET calculation_json = JSON_SET(calculation_json,'$.profit',CAST(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.isv_15')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.isv_18')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.investment_total')) AS DECIMAL(30,2)) AS CHAR))
WHERE JSON_EXTRACT(calculation_json,'$.totals.total') IS NOT NULL
  AND JSON_EXTRACT(calculation_json,'$.investment_total') IS NOT NULL
  AND JSON_EXTRACT(calculation_json,'$.totals.isv_15') IS NOT NULL
  AND JSON_EXTRACT(calculation_json,'$.totals.isv_18') IS NOT NULL;

UPDATE facturas_create_receipts
SET response_json = JSON_SET(response_json,'$.calculation.profit',CAST(CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.total')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.isv_15')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.isv_18')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.investment_total')) AS DECIMAL(30,2)) AS CHAR))
WHERE JSON_EXTRACT(response_json,'$.calculation.totals.total') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.investment_total') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.totals.isv_15') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.totals.isv_18') IS NOT NULL;
