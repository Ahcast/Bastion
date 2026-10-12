ALTER TABLE invoices
 ADD source_quote_id BIGINT UNSIGNED NULL,
 ADD source_quote_version BIGINT UNSIGNED NULL,
 ADD UNIQUE KEY invoice_source_quote(source_quote_id),
 ADD FOREIGN KEY(source_quote_id) REFERENCES quotes(id);
ALTER TABLE quotes
 ADD invoice_id BIGINT UNSIGNED NULL,
 ADD UNIQUE KEY quote_invoice(invoice_id),
 ADD FOREIGN KEY(invoice_id) REFERENCES invoices(id);
