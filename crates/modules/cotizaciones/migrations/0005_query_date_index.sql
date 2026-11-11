-- Preserve the original JSON value and ordering semantics. The derived column
-- is maintained by MySQL on create, edit and clone; no application dual writes.
ALTER TABLE cotizaciones_drafts
    ADD COLUMN quote_date_search VARCHAR(10)
        GENERATED ALWAYS AS (JSON_UNQUOTE(JSON_EXTRACT(data_json,'$.quote_date'))) STORED,
    ADD KEY cotizaciones_date_id (quote_date_search, id);
