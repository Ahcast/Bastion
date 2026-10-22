-- Derived UTC timestamp for weekly discovery counts. Original CSV text stays in source_json.
ALTER TABLE procesos_rows ADD primera_deteccion DATETIME(6) NULL AFTER ultima_deteccion;

UPDATE procesos_rows
SET primera_deteccion = TIMESTAMPADD(
  MINUTE,
  CASE
    WHEN RIGHT(JSON_UNQUOTE(JSON_EXTRACT(source_json,'$.primera_deteccion')),1) IN ('Z','z') THEN 0
    ELSE
      (CASE WHEN SUBSTRING(JSON_UNQUOTE(JSON_EXTRACT(source_json,'$.primera_deteccion')),-6,1)='+' THEN -1 ELSE 1 END)
      * (CAST(SUBSTRING(JSON_UNQUOTE(JSON_EXTRACT(source_json,'$.primera_deteccion')),-5,2) AS UNSIGNED)*60
         + CAST(RIGHT(JSON_UNQUOTE(JSON_EXTRACT(source_json,'$.primera_deteccion')),2) AS UNSIGNED))
  END,
  STR_TO_DATE(CONCAT(
    LEFT(JSON_UNQUOTE(JSON_EXTRACT(source_json,'$.primera_deteccion')),10),' ',
    SUBSTRING(JSON_UNQUOTE(JSON_EXTRACT(source_json,'$.primera_deteccion')),12,8)
  ),'%Y-%m-%d %H:%i:%s')
);

ALTER TABLE procesos_rows
  MODIFY primera_deteccion DATETIME(6) NOT NULL,
  ADD KEY ix_procesos_primera_deteccion (primera_deteccion,proceso_id);
