-- CastilloIT preserves deleted expenses rather than erasing their data.
ALTER TABLE gastos_records
    ADD COLUMN deleted_at DATETIME(6) NULL,
    ADD KEY gastos_active_date (deleted_at, expense_date, id);
