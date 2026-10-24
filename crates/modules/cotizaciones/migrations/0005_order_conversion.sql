
CREATE TABLE cotizaciones_order_conversions (
    quote_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    source_version BIGINT UNSIGNED NOT NULL,
    order_id BIGINT UNSIGNED NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    UNIQUE KEY cotizaciones_converted_order (order_id),
    CONSTRAINT cotizaciones_conversion_quote_fk FOREIGN KEY(quote_id) REFERENCES cotizaciones_drafts(id),
    CONSTRAINT cotizaciones_conversion_actor_fk FOREIGN KEY(created_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
