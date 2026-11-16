CREATE TABLE contabilidad_accounts (
 code VARCHAR(12) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
 name VARCHAR(120) NOT NULL, class VARCHAR(16) CHARACTER SET ascii NOT NULL,
 parent_code VARCHAR(12) CHARACTER SET ascii COLLATE ascii_bin NULL,
 posting BOOLEAN NOT NULL, active BOOLEAN NOT NULL DEFAULT 1, version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 FOREIGN KEY (parent_code) REFERENCES contabilidad_accounts(code)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
CREATE TABLE contabilidad_periods (
 month CHAR(7) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
 closed BOOLEAN NOT NULL DEFAULT 0, version BIGINT UNSIGNED NOT NULL DEFAULT 1
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
CREATE TABLE contabilidad_entries (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 entry_date DATE NOT NULL, month CHAR(7) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 description VARCHAR(240) NOT NULL, reference VARCHAR(120) NOT NULL,
 kind VARCHAR(16) NOT NULL, status VARCHAR(16) NOT NULL DEFAULT 'borrador', total_cents BIGINT NOT NULL,
 source_key VARCHAR(100) CHARACTER SET ascii COLLATE ascii_bin NULL, source_json JSON NULL,
 reversal_of BIGINT UNSIGNED NULL, reversed_by BIGINT UNSIGNED NULL,
 created_by BIGINT UNSIGNED NOT NULL, updated_by BIGINT UNSIGNED NOT NULL,
 idempotency_key VARCHAR(80) CHARACTER SET ascii COLLATE ascii_bin NOT NULL, request_hash BINARY(32) NOT NULL,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6), updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 UNIQUE KEY uq_accounting_request(created_by,idempotency_key), UNIQUE KEY uq_accounting_source(source_key), UNIQUE KEY uq_accounting_reversal(reversal_of),
 KEY ix_accounting_date(entry_date,id), KEY ix_accounting_month(month,status),
 FOREIGN KEY(month) REFERENCES contabilidad_periods(month), FOREIGN KEY(reversal_of) REFERENCES contabilidad_entries(id),
 FOREIGN KEY(created_by) REFERENCES users(id), FOREIGN KEY(updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
CREATE TABLE contabilidad_lines (
 entry_id BIGINT UNSIGNED NOT NULL, line_number INT UNSIGNED NOT NULL,
 account_code VARCHAR(12) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 debit_cents BIGINT NOT NULL, credit_cents BIGINT NOT NULL,
 PRIMARY KEY(entry_id,line_number), KEY ix_accounting_account(account_code,entry_id),
 FOREIGN KEY(entry_id) REFERENCES contabilidad_entries(id), FOREIGN KEY(account_code) REFERENCES contabilidad_accounts(code)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
CREATE TABLE contabilidad_actor_limits (
 actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY, window_start DATETIME(6) NOT NULL, requests INT UNSIGNED NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
INSERT INTO contabilidad_accounts(code,name,class,parent_code,posting) VALUES
('1','Activos','activo',NULL,0),('2','Pasivos','pasivo',NULL,0),('3','Patrimonio','patrimonio',NULL,0),('4','Ingresos','ingreso',NULL,0),('5','Costos','costo',NULL,0),('6','Gastos','gasto',NULL,0),
('11','Activo corriente','activo','1',0),('12','Activos de uso','activo','1',0),('21','Obligaciones corrientes','pasivo','2',0),('22','Obligaciones de largo plazo','pasivo','2',0),('31','Capital y resultados','patrimonio','3',0),('41','Ventas y otros ingresos','ingreso','4',0),('51','Costos de venta','costo','5',0),('61','Gastos operativos','gasto','6',0),
('1101','Caja y caja chica','activo','11',1),('1102','Bancos HNL','activo','11',1),('1103','Bancos en moneda extranjera — equivalente HNL','activo','11',1),('1104','Clientes por cobrar','activo','11',1),('1105','Inventario de mercadería','activo','11',1),('1106','Compras en tránsito / casillero','activo','11',1),('1107','Anticipos a proveedores','activo','11',1),('1108','ISV crédito fiscal sujeto a revisión','activo','11',1),('1109','Gastos pagados por anticipado','activo','11',1),('1110','Anticipos a empleados','activo','11',1),
('1201','Vehículos','activo','12',1),('1202','Computadoras y equipo','activo','12',1),('1203','Mobiliario','activo','12',1),('1291','Depreciación acumulada de vehículos','activo','12',1),('1292','Depreciación acumulada de equipo','activo','12',1),
('2101','Proveedores por pagar','pasivo','21',1),('2102','Tarjetas y préstamos de corto plazo','pasivo','21',1),('2103','Sueldos por pagar','pasivo','21',1),('2104','IHSS por pagar','pasivo','21',1),('2105','RAP y reserva laboral por pagar','pasivo','21',1),('2106','ISR retenido por pagar','pasivo','21',1),('2107','ISV trasladado por pagar','pasivo','21',1),('2108','Beneficios laborales acumulados','pasivo','21',1),('2109','Anticipos de clientes','pasivo','21',1),('2110','Otros impuestos y aportaciones por pagar','pasivo','21',1),('2201','Préstamos de largo plazo','pasivo','22',1),
('3101','Capital y aportaciones','patrimonio','31',1),('3102','Resultados acumulados','patrimonio','31',1),('3103','Retiros o distribuciones a propietarios','patrimonio','31',1),
('4101','Ventas de mercadería','ingreso','41',1),('4102','Ingresos por servicios','ingreso','41',1),('4103','Otros ingresos','ingreso','41',1),('5101','Costo de mercadería vendida','costo','51',1),
('6101','Sueldos y salarios','gasto','61',1),('6102','Aportaciones patronales','gasto','61',1),('6103','Décimos, vacaciones y beneficios laborales','gasto','61',1),('6104','Alquiler','gasto','61',1),('6105','Energía, agua, internet y telefonía','gasto','61',1),('6106','Suscripciones y software','gasto','61',1),('6107','Combustible','gasto','61',1),('6108','Mantenimiento de vehículo y equipo','gasto','61',1),('6109','Seguros','gasto','61',1),('6110','Honorarios y servicios','gasto','61',1),('6111','Papelería y suministros','gasto','61',1),('6112','Mensajería y fletes de venta','gasto','61',1),('6113','Comisiones e intereses','gasto','61',1),('6114','Depreciación','gasto','61',1),('6115','Otros gastos operativos','gasto','61',1),('6116','Diferencias cambiarias — pérdida','gasto','61',1),('4104','Diferencias cambiarias — ganancia','ingreso','41',1);
