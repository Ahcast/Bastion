ALTER TABLE cotizaciones_drafts
    MODIFY client_name VARCHAR(255) NOT NULL,
    ADD COLUMN quote_number VARCHAR(50) NULL,
    ADD COLUMN calculation_json JSON NULL,
    ADD UNIQUE KEY cotizaciones_quote_number (quote_number);

CREATE TABLE cotizaciones_counter (
    id TINYINT UNSIGNED NOT NULL PRIMARY KEY,
    last_number BIGINT UNSIGNED NOT NULL
) ENGINE=InnoDB;

INSERT INTO cotizaciones_counter (id,last_number) VALUES (1,0);
