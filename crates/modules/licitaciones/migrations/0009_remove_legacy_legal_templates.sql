-- Remove only the two superseded legal templates; preserve all other selections.
UPDATE licitaciones_expedientes
SET initial_forms_json = JSON_REMOVE(initial_forms_json, JSON_UNQUOTE(JSON_SEARCH(initial_forms_json, 'one', 'integridad'))),
    version = version + 1, updated_at = UTC_TIMESTAMP(6)
WHERE JSON_SEARCH(initial_forms_json, 'one', 'integridad') IS NOT NULL;

UPDATE licitaciones_expedientes
SET initial_forms_json = JSON_REMOVE(initial_forms_json, JSON_UNQUOTE(JSON_SEARCH(initial_forms_json, 'one', 'oferente'))),
    version = version + 1, updated_at = UTC_TIMESTAMP(6)
WHERE JSON_SEARCH(initial_forms_json, 'one', 'oferente') IS NOT NULL;

DELETE FROM licitaciones_formularios_instalados WHERE form_key IN ('integridad', 'oferente');
