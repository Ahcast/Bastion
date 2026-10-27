-- User decision 2026-10-26: profit = total sale - ISV - total investment.
-- Correct only derived profit; preserve source inputs, totals, versions and origins.
UPDATE ordenes_orders
SET calculation_json = JSON_SET(calculation_json,'$.totals.profit',CAST(CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.total')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.tax')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(calculation_json,'$.totals.investment_total')) AS DECIMAL(30,2)) AS CHAR))
WHERE JSON_EXTRACT(calculation_json,'$.totals.total') IS NOT NULL
  AND JSON_EXTRACT(calculation_json,'$.totals.investment_total') IS NOT NULL
  AND JSON_EXTRACT(calculation_json,'$.totals.tax') IS NOT NULL;

UPDATE ordenes_create_receipts
SET response_json = JSON_SET(response_json,'$.calculation.totals.profit',CAST(CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.total')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.tax')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.investment_total')) AS DECIMAL(30,2)) AS CHAR))
WHERE JSON_EXTRACT(response_json,'$.calculation.totals.total') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.totals.investment_total') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.totals.tax') IS NOT NULL;

UPDATE ordenes_clone_receipts
SET response_json = JSON_SET(response_json,'$.calculation.totals.profit',CAST(CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.total')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.tax')) AS DECIMAL(30,2)) - CAST(JSON_UNQUOTE(JSON_EXTRACT(response_json,'$.calculation.totals.investment_total')) AS DECIMAL(30,2)) AS CHAR))
WHERE JSON_EXTRACT(response_json,'$.calculation.totals.total') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.totals.investment_total') IS NOT NULL
  AND JSON_EXTRACT(response_json,'$.calculation.totals.tax') IS NOT NULL;
