use std::collections::{BTreeMap, BTreeSet};

use bastion_mcp::{RegistryError, ToolDescriptor, ToolRegistry, ToolRisk};
use serde_json::json;

use crate::PermissionRequirement;

const READ_MODULES: [(&str, &str, &str); 9] = [
    ("cotizaciones", "cotizaciones", "documentos"),
    ("facturas", "facturas", "documentos"),
    ("ordenes_compra", "órdenes de compra", "documentos"),
    ("licitaciones", "licitaciones", "expedientes"),
    ("procesos", "procesos vigentes", "consulta"),
    ("clientes", "clientes", "catalogo"),
    ("proveedores", "proveedores", "catalogo"),
    ("gastos", "gastos", "registros"),
    (
        "colaboracion",
        "archivos de empresa (solo metadatos)",
        "empresa",
    ),
];

pub(crate) const REPORTS: [(&str, &str, &str); 3] = [
    ("ordenes", "ordenes_compra", "documentos"),
    ("facturas", "facturas", "documentos"),
    ("gastos", "gastos", "registros"),
];

/// User-approved read-only modules. A scope does not grant permissions: the
/// Core still checks the user's current `documentos/ver` permission, module
/// status and grant on every invocation.
pub fn approved_read_requirements() -> BTreeMap<String, Vec<PermissionRequirement>> {
    READ_MODULES
        .into_iter()
        .map(|(module, _, scope_key)| {
            (
                format!("{module}:read"),
                vec![PermissionRequirement {
                    module_id: module.into(),
                    scope_key: scope_key.into(),
                    capability: "ver".into(),
                }],
            )
        })
        .chain(REPORTS.map(|(report, module, scope_key)| {
            (
                format!("reportes:{report}:read"),
                vec![
                    PermissionRequirement {
                        module_id: "reportes".into(),
                        scope_key: report.into(),
                        capability: "ver".into(),
                    },
                    PermissionRequirement {
                        module_id: module.into(),
                        scope_key: scope_key.into(),
                        capability: "ver".into(),
                    },
                ],
            )
        }))
        .chain(
            ["cotizaciones", "facturas", "ordenes_compra", "licitaciones"]
                .into_iter()
                .map(|module| {
                    let scope_key = if module == "licitaciones" {
                        "expedientes"
                    } else {
                        "documentos"
                    };
                    (
                        format!("{module}:write"),
                        ["ver", "editar"]
                            .into_iter()
                            .map(|capability| PermissionRequirement {
                                module_id: module.into(),
                                scope_key: scope_key.into(),
                                capability: capability.into(),
                            })
                            .collect(),
                    )
                }),
        )
        .collect()
}

/// Descriptors only. Do not add these to a live router until typed dispatch,
/// per-object authorization, quotas, output limits and auditing are wired.
pub fn draft_read_tool_registry() -> Result<ToolRegistry, RegistryError> {
    let mut tools = Vec::new();
    for (module, label, _) in READ_MODULES {
        let scope = format!("{module}:read");
        tools.push(ToolDescriptor {
            name: format!("{module}.listar"), module_id:module.into(),
            description:format!("Consultar un listado paginado de {label} autorizado para el usuario."),
            required_scope:scope.clone(),
            input_schema:if module == "licitaciones" { json!({"type":"object","properties":{"page":{"type":"integer","minimum":1,"maximum":10000},"page_size":{"type":"integer","minimum":1,"maximum":25}},"additionalProperties":false}) } else { json!({"type":"object","properties":{"search":{"type":"string","maxLength":200},"page":{"type":"integer","minimum":1,"maximum":10000},"page_size":{"type":"integer","minimum":1,"maximum":25}},"additionalProperties":false}) },
            output_schema:None,risk:ToolRisk::ReadOnly,idempotent:true,max_response_bytes:262144,max_duration_ms:10000,
        });
        if module == "colaboracion" {
            continue;
        }
        let id = if module == "procesos" {
            json!({"type":"string","minLength":67,"maxLength":67})
        } else {
            json!({"type":"integer","minimum":1,"maximum":9007199254740991_u64})
        };
        tools.push(ToolDescriptor {
            name:format!("{module}.consultar"),module_id:module.into(),
            description:format!("Consultar una de las {label} por su identificador, sujeto a los permisos del usuario."),
            required_scope:scope,
            input_schema:json!({"type":"object","properties":{"id":id},"required":["id"],"additionalProperties":false}),
            output_schema:None,risk:ToolRisk::ReadOnly,idempotent:true,max_response_bytes:262144,max_duration_ms:10000,
        });
    }
    for (report, _, _) in REPORTS {
        tools.push(ToolDescriptor {
            name: format!("reportes.{report}"),
            module_id: "reportes".into(),
            description: format!("Consultar el reporte paginado de {report} por rango de fechas."),
            required_scope: format!("reportes:{report}:read"),
            input_schema: json!({"type":"object","properties":{
                "date_from":{"type":"string","format":"date"},
                "date_to":{"type":"string","format":"date"},
                "search":{"type":"string","maxLength":200},
                "page":{"type":"integer","minimum":1,"maximum":10000},
                "page_size":{"type":"integer","minimum":1,"maximum":25}
            },"required":["date_from","date_to"],"additionalProperties":false}),
            output_schema: None,
            risk: ToolRisk::ReadOnly,
            idempotent: true,
            max_response_bytes: 262144,
            max_duration_ms: 10000,
        });
    }
    for (operation, description, schema) in [
        (
            "listar_lotes",
            "Consultar los lotes de un expediente.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1}},"required":["id"],"additionalProperties":false}),
        ),
        (
            "consultar_lote",
            "Consultar un lote y sus partidas.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"numero":{"type":"integer","minimum":1}},"required":["id","numero"],"additionalProperties":false}),
        ),
    ] {
        tools.push(ToolDescriptor {
            name: format!("licitaciones.{operation}"),
            module_id: "licitaciones".into(),
            description: description.into(),
            required_scope: "licitaciones:read".into(),
            input_schema: schema,
            output_schema: None,
            risk: ToolRisk::ReadOnly,
            idempotent: true,
            max_response_bytes: 262144,
            max_duration_ms: 10000,
        });
    }
    for (module, operation, description, mut schema, idempotent) in [
        (
            "cotizaciones",
            "crear_borrador",
            "Crear una cotización en borrador.",
            json!({"type":"object","properties":{"idempotency_key":{"type":"string","minLength":16,"maxLength":128},"data":{"type":"object","description":"Datos completos de cotización; status debe ser Borrador."}},"required":["idempotency_key","data"],"additionalProperties":false}),
            true,
        ),
        (
            "cotizaciones",
            "editar_borrador",
            "Reemplazar los datos de una cotización en borrador con la versión observada.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"version":{"type":"integer","minimum":1},"data":{"type":"object","description":"Datos completos de cotización; status debe ser Borrador."}},"required":["id","version","data"],"additionalProperties":false}),
            false,
        ),
        (
            "facturas",
            "crear_borrador",
            "Crear una factura en borrador sin registrar pagos.",
            json!({"type":"object","properties":{"idempotency_key":{"type":"string","minLength":16,"maxLength":128},"data":{"type":"object","description":"Datos completos de factura, sin initial_payment."}},"required":["idempotency_key","data"],"additionalProperties":false}),
            true,
        ),
        (
            "facturas",
            "editar_borrador",
            "Reemplazar los datos de una factura en borrador sin registrar pagos.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"version":{"type":"integer","minimum":1},"data":{"type":"object","description":"Datos completos de factura, sin initial_payment."}},"required":["id","version","data"],"additionalProperties":false}),
            false,
        ),
        (
            "ordenes_compra",
            "crear_pendiente",
            "Crear una orden pendiente sin compras ni entrega.",
            json!({"type":"object","properties":{"idempotency_key":{"type":"string","minLength":16,"maxLength":128},"data":{"type":"object","description":"Datos de la orden, sin factura ni fecha de entrega; todos los productos en Pendiente."}},"required":["idempotency_key","data"],"additionalProperties":false}),
            true,
        ),
        (
            "ordenes_compra",
            "editar_pendiente",
            "Editar una orden pendiente sin compras ni entrega.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"version":{"type":"integer","minimum":1},"data":{"type":"object","description":"Datos de la orden, sin factura ni fecha de entrega; todos los productos en Pendiente."}},"required":["id","version","data"],"additionalProperties":false}),
            false,
        ),
        (
            "licitaciones",
            "crear_borrador",
            "Crear un expediente de licitación vacío en borrador.",
            json!({"type":"object","properties":{"idempotency_key":{"type":"string","minLength":16,"maxLength":128}},"required":["idempotency_key"],"additionalProperties":false}),
            true,
        ),
        (
            "licitaciones",
            "renombrar_borrador",
            "Cambiar el nombre de un expediente en borrador.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"version":{"type":"integer","minimum":1},"name":{"type":"string","maxLength":255}},"required":["id","version","name"],"additionalProperties":false}),
            false,
        ),
        (
            "licitaciones",
            "editar_variables",
            "Reemplazar las variables de un expediente en borrador, sin descartar lotes.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"version":{"type":"integer","minimum":1},"variables":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"properties":{"key":{"type":"string"},"label":{"type":"string"},"kind":{"enum":["text","integer","decimal","date","boolean"]},"value":{}},"required":["key","label","kind"]}}},"required":["id","version","variables"],"additionalProperties":false}),
            false,
        ),
        (
            "licitaciones",
            "editar_lote",
            "Reemplazar las partidas de un lote de un expediente en borrador.",
            json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"version":{"type":"integer","minimum":1},"numero":{"type":"integer","minimum":1},"items":{"type":"array","maxItems":1000,"items":{"type":"object","additionalProperties":false,"properties":{"descripcion":{"type":"string"},"pais_origen":{"type":"string"},"fecha_entrega":{"type":"string"},"cantidad":{"type":"string"},"unidad":{"type":"string"},"transporte":{"type":"string"},"precio_unitario":{"type":"string"}},"required":["descripcion","pais_origen","fecha_entrega","cantidad","unidad","transporte","precio_unitario"]}}},"required":["id","version","numero","items"],"additionalProperties":false}),
            false,
        ),
    ] {
        if matches!(module, "cotizaciones" | "facturas" | "ordenes_compra") {
            schema["properties"]["data"] = draft_data_schema(module);
        }
        tools.push(ToolDescriptor {
            name: format!("{module}.{operation}"),
            module_id: module.into(),
            description: description.into(),
            required_scope: format!("{module}:write"),
            input_schema: schema,
            output_schema: None,
            risk: ToolRisk::Mutation,
            idempotent,
            max_response_bytes: 262144,
            max_duration_ms: 10000,
        });
    }
    tools.push(ToolDescriptor {
        name: "ordenes_compra.listar_destinos".into(),
        module_id: "ordenes_compra".into(),
        description: "Consultar los destinos disponibles para órdenes de compra.".into(),
        required_scope: "ordenes_compra:read".into(),
        input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
        output_schema: None,
        risk: ToolRisk::ReadOnly,
        idempotent: true,
        max_response_bytes: 262144,
        max_duration_ms: 10000,
    });
    let scopes: BTreeSet<String> = approved_read_requirements().into_keys().collect();
    ToolRegistry::new(tools, &scopes)
}

fn draft_data_schema(module: &str) -> serde_json::Value {
    match module {
        "cotizaciones" => json!({"type":"object","additionalProperties":false,"properties":{
            "id_client":{"type":["integer","null"]},"client_name":{"type":"string"},"client_rtn":{"type":"string"},"client_email":{"type":"string"},"order_number":{"type":"string"},"quote_date":{"type":"string","format":"date"},"valid_days":{"type":"integer"},"status":{"const":"Borrador"},"contact_email":{"type":"string"},"prepared_by_name":{"type":"string"},"purchase_order_exempt_number":{"type":"string"},"registration_certificate_number":{"type":"string"},"sag_registration_number":{"type":"string"},"notes":{"type":"string"},"terms":{"type":"string"},"discount_type":{"enum":["amount","percent"]},"discount_value":{"type":"string"},"items":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"description":{"type":"string"},"quantity":{"type":"string"},"unit_price":{"type":"string"},"discount_type":{"enum":["amount","percent"]},"discount_value":{"type":"string"},"tax_type":{"enum":["none","isv15","isv18"]}},"required":["description","quantity","unit_price","discount_type","discount_value","tax_type"]}}},"required":["client_name","order_number","quote_date","valid_days","notes","terms","discount_type","discount_value","items"]}),
        "facturas" => json!({"type":"object","additionalProperties":false,"properties":{
            "id_client":{"type":["integer","null"]},"client_name":{"type":"string"},"client_rtn":{"type":"string"},"client_email":{"type":"string"},"order_number":{"type":"string"},"invoice_date":{"type":"string","format":"date"},"payment_term_type":{"enum":["contado","credito"]},"credit_days":{"type":"integer"},"discount_type":{"enum":["amount","percent"]},"discount_value":{"type":"string"},"purchase_order_exempt_number":{"type":"string"},"registration_certificate_number":{"type":"string"},"sag_registration_number":{"type":"string"},"notes":{"type":"string"},"public_notes":{"type":"string"},"items":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"description":{"type":"string"},"quantity":{"type":"string"},"unit_price":{"type":"string"},"cost_price":{"type":"string"},"discount_type":{"enum":["amount","percent"]},"discount_value":{"type":"string"},"tax_type":{"enum":["none","isv15","isv18"]}},"required":["description","quantity","unit_price","cost_price","discount_type","discount_value","tax_type"]}},"expenses":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"description":{"type":"string"},"amount":{"type":"string"}},"required":["description","amount"]}}},"required":["client_name","order_number","invoice_date","payment_term_type","credit_days","discount_type","discount_value","notes","public_notes","items","expenses"]}),
        _ => json!({"type":"object","additionalProperties":false,"properties":{
            "n_orders":{"type":"string"},"n_bill":{"const":""},"order_date":{"type":"string","format":"date"},"received_date":{"type":"null"},"client_name":{"type":"string"},"client_rtn":{"type":"string"},"id_destination":{"type":["integer","null"]},"comments":{"type":"string"},"products":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"line_id":{"type":"string"},"description":{"type":"string"},"cantidad":{"type":"integer"},"sale_price":{"type":"string","description":"Costo"},"purchase_price":{"type":"string","description":"Precio de venta"},"tracking":{"type":"string"},"isv":{"type":"boolean"},"status":{"const":"Pendiente"},"coments":{"type":"string"}},"required":["description","cantidad","sale_price","purchase_price","tracking","isv","status","coments"]}},"expenses":{"type":"array","items":{"type":"object","additionalProperties":false,"properties":{"description":{"type":"string"},"amount":{"type":"string"}},"required":["description","amount"]}}},"required":["n_orders","n_bill","order_date","received_date","client_name","client_rtn","id_destination","comments","products","expenses"]}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bastion_mcp::AuthenticatedAccess;

    #[test]
    fn exposes_only_approved_read_tools_for_active_modules_and_scopes() {
        let requirements = approved_read_requirements();
        assert_eq!(requirements.len(), 16);
        let process = &requirements["procesos:read"][0];
        assert_eq!(process.scope_key, "consulta");
        assert_eq!(process.capability, "ver");
        let report = &requirements["reportes:gastos:read"];
        assert_eq!(report[0].module_id, "reportes");
        assert_eq!(report[1].module_id, "gastos");
        let registry = draft_read_tool_registry().unwrap();
        let user = AuthenticatedAccess {
            user_id: 1,
            client_id: 2,
            grant_id: 3,
            scopes: BTreeSet::from(["cotizaciones:read".into(), "facturas:read".into()]),
        };
        let modules = BTreeSet::from(["cotizaciones".into(), "ordenes_compra".into()]);
        let visible = registry.visible(&user, &modules, false);
        assert_eq!(visible.len(), 2);
        assert!(
            visible
                .iter()
                .all(|t| t.module_id == "cotizaciones" && t.risk == ToolRisk::ReadOnly)
        );
        assert!(registry.visible(&user, &BTreeSet::new(), false).is_empty());
    }
}
