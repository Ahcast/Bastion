// Compile reviewed API contracts to inert native form metadata. Never executes server content.
const fs=require('node:fs'),path=require('node:path');
const yaml=require(process.env.BASTION_YAML || 'yaml');
const root=path.resolve(__dirname,'../..');
const operations=[];
const moduleIds={'ordenes-compra':'ordenes_compra'};
const modules=[['clientes','Clientes','catalogo'],['proveedores','Proveedores','catalogo'],['procesos','Procesos','consulta'],['cotizaciones','Cotizaciones','documentos'],['ordenes_compra','Órdenes de compra','documentos'],['facturas','Facturas','documentos'],['gastos','Gastos','registros'],['contabilidad','Contabilidad','libros'],['reportes','Reportes','*'],['licitaciones','Licitaciones','expedientes'],['colaboracion','Colaboración','*'],['core','Ajustes','*']].map(([id,title,scope])=>({id,title,scope}));
function resolve(value,doc,seen=[]){
 if(!value||typeof value!=='object')return value;
 if(Array.isArray(value))return value.map(v=>resolve(v,doc,seen));
 if(value.$ref){if(seen.includes(value.$ref))throw Error('Recursive schema '+value.$ref);let [file,fragment]=value.$ref.split('#'),referenceDoc=file?yaml.parse(fs.readFileSync(path.join(root,'docs/modulos',file),'utf8')):doc;let target=referenceDoc;for(const p of fragment.slice(1).split('/'))target=target?.[p];if(!target)throw Error('Missing '+value.$ref);return resolve({...target,...Object.fromEntries(Object.entries(value).filter(([k])=>k!=='$ref'))},referenceDoc,[...seen,value.$ref]);}
 const out=Object.fromEntries(Object.entries(value).filter(([k,v])=>!(k==='description'&&typeof v==='string')&&!['example','examples','responses','security'].includes(k)).map(([k,v])=>[k,resolve(v,doc,seen)]));
 if(out.allOf){for(const s of out.allOf){out.properties={...out.properties,...s.properties};out.required=[...new Set([...(out.required||[]),...(s.required||[])])];}out.type='object';delete out.allOf;}
 if(out.anyOf){const nonnull=out.anyOf.filter(s=>s.type!=='null');if(nonnull.length===1){Object.assign(out,nonnull[0],{nullable:nonnull.length!==out.anyOf.length});delete out.anyOf;}}
 return out;
}
const names={attachments:'Adjuntos',archivos:'Archivos',emails:'Correos',status:'Estado',clone:'Duplicar',pdf:'PDF',vista:'Vista previa',calculate:'Calcular',calculo:'Calcular',catalogo:'Catálogo',dashboard:'Resumen',payments:'Registrar pago','mark-sent':'Emitir factura',documents:'Documentos',destinations:'Destinos',invoice:'Convertir a factura','convert-invoice':'Convertir a factura','convert-order':'Convertir a orden','order-conversion':'Preparar orden',preview:'Vista previa',download:'Descargar',content:'Descargar',restore:'Restaurar',purge:'Eliminar definitivamente',settings:'Configuración',personas:'Personas',formularios:'Formularios',variables:'Variables',lotes:'Lotes',adjudicaciones:'Adjudicaciones',adjudicacion:'Adjudicación','marcar-enviada':'Marcar enviada','plantillas-instaladas':'Plantillas',orden:'Orden del PDF','formularios-iniciales':'Formularios iniciales',generar:'Generar PDF',ultimo:'Último PDF',archivo:'Descargar PDF',valores:'Valores del formulario','desde-colaboracion':'Copiar desde Colaboración',rows:'Procesos',review:'Marcas y notas',refresh:'Importar procesos','scout-run':'Ejecutar Scout','scout-status':'Estado de Scout',entities:'Instituciones',export:'Exportar',cuentas:'Cuentas',periodos:'Períodos',asientos:'Asientos',origen:'Propuesta desde documento',balance:'Balance de comprobación',cancel:'Cancelar correo',retry:'Reintentar correo','document-readiness':'Disponibilidad de documentos',factura:'Factura de adjudicación'};
function title(method,p){let parts=p.split('/').slice(3),last=parts.at(-1);if(last.startsWith('{')){if(method==='GET')return 'Detalle';if(method==='DELETE')return 'Eliminar';return method==='POST'?(names[parts.at(-2)]||'Continuar'):'Editar';}if(parts.length===1)return method==='GET'?'Listado':method==='POST'?'Crear':'Editar';let name=names[last]||last;return method==='GET'?name:method==='PUT'||method==='PATCH'?'Editar '+name:method==='DELETE'?'Eliminar '+name:name;}
for(const file of fs.readdirSync(path.join(root,'docs/modulos')).filter(f=>f.endsWith('.openapi.yaml')).sort()){
 const doc=yaml.parse(fs.readFileSync(path.join(root,'docs/modulos',file),'utf8'));
 for(const [p,def] of Object.entries(doc.paths||{}))for(const method of ['get','post','put','patch','delete'])if(def[method]){
  const op=def[method],module=moduleIds[p.split('/')[3]]||p.split('/')[3];
  const params=resolve([...(def.parameters||[]),...(op.parameters||[])],doc);
  for(const match of p.matchAll(/\{([^}]+)\}/g))if(!params.some(v=>v.in==='path'&&v.name===match[1]))params.push({in:'path',name:match[1],required:true,schema:{type:match[1]==='id'?'integer':'string'}});
  let scope=modules.find(m=>m.id===module)?.scope||'*';
  if(module==='reportes')scope=p.split('/')[4];
  if(module==='ordenes_compra'&&p.includes('/destinations'))scope='destinos';
  if(module==='colaboracion'&&p.endsWith('/settings'))scope='settings';
  let permission=op['x-permission'];if(!permission||!/^\w+\/[\w*]+\/\w+$/.test(permission))permission=module+'/'+scope+'/'+(method==='get'?'ver':method==='delete'?'eliminar':'editar');
  if(module==='procesos'&&/refresh|scout-run/.test(p)&&method==='post')permission='procesos/importacion/editar';
  if(module==='procesos'&&p.endsWith('/review'))permission='procesos/marcas/editar|procesos/notas/editar';
  if(module==='colaboracion'&&/restore|purge/.test(p))permission='colaboracion/*/eliminar';
  const body=resolve(op.requestBody,doc)?.content?.['application/json']?.schema;
  operations.push({path:p,method:method.toUpperCase(),module,title:title(method.toUpperCase(),p),permission,parameters:params,...(body?{body:resolve(body,doc)}:{}),source:file});
 }
}
const str=(extra={})=>({type:'string',...extra}),int=(extra={})=>({type:'integer',...extra}),bool={type:'boolean'},arr=items=>({type:'array',items}),obj=(properties,required=Object.keys(properties))=>({type:'object',properties,required}),en=(...values)=>str({enum:values}),date=str({format:'date'}),optional=s=>({...s,nullable:true});
function op(method,p,body,parameters=[],permission='core/*/ver',label){let o=operations.find(o=>o.path===p&&o.method===method);if(!o){o={path:p,method,module:'core',title:label||title(method,p),permission,parameters:[],source:'crates/api/src/lib.rs'};operations.push(o);}if(body)o.body=body;o.parameters.push(...parameters);if(label)o.title=label;return o;}
const query=(name,schema,required=false)=>({name,schema,in:'query',required});
const account=obj({code:str(),name:str(),class:en('activo','pasivo','patrimonio','ingreso','costo','gasto'),parent_code:str(),posting:bool});
const source=obj({module:en('gastos','facturas','ordenes_compra'),id:int(),version:int()});
const entry=obj({date,description:str(),reference:str(),kind:str({default:'manual'}),source:optional(source),lines:{...arr(obj({account:str(),debit:str({default:'0.00'}),credit:str({default:'0.00'})})),minItems:2,maxItems:100}});
op('POST','/api/v1/contabilidad/cuentas',account);
op('PUT','/api/v1/contabilidad/cuentas/{code}',obj({name:str(),active:bool}));
op('POST','/api/v1/contabilidad/periodos',obj({month:str({pattern:'^\\d{4}-\\d{2}$'})}));
op('PUT','/api/v1/contabilidad/periodos/{month}',obj({closed:bool,reason:str()}));
op('POST','/api/v1/contabilidad/asientos',entry);op('PUT','/api/v1/contabilidad/asientos/{id}',entry);
op('POST','/api/v1/contabilidad/asientos/{id}/{action}',obj({date:optional(date),reason:optional(str())},[]));
op('GET','/api/v1/contabilidad/asientos',null,[query('month',str()),query('page',int({default:1}))]);
op('GET','/api/v1/contabilidad/balance',null,[query('date_from',date,true),query('date_to',date,true)]);
const profile=obj({full_name:str(),dni:optional(str()),country_code:optional(str()),gender:optional(str())},['full_name']);
for(const [resource,label] of [['profile','Mi perfil'],['sessions','Sesiones'],['devices','Dispositivos'],['security-events','Seguridad'],['smtp','Mi correo']])op('GET','/api/v1/account/'+resource,null,[],'account',label);
op('PUT','/api/v1/account/profile',profile,[],'account','Editar perfil');
op('PUT','/api/v1/account/smtp',obj({smtp_email:str({format:'email'}),smtp_password:optional(str({format:'password'}))}),[],'account','Configurar correo');
op('POST','/api/v1/account/password',obj({current_password:str({format:'password'}),new_password:str({format:'password'}),totp_code:str()}),[],'account','Cambiar contraseña');
op('POST','/api/v1/account/totp/change',obj({current_password:str({format:'password'}),current_code:str()}),[],'account','Cambiar TOTP');
op('POST','/api/v1/account/totp/change/complete',obj({setup_token:str(),code:str()}),[],'account','Confirmar TOTP');
for(const resource of ['sessions','devices'])op('DELETE','/api/v1/account/'+resource+'/{id}',null,[],'account','Revocar');
op('POST','/api/v1/account/sessions/revoke-others',null,[],'account','Cerrar otras sesiones');
op('GET','/api/v1/persons',null,[query('q',str()),query('page',int({default:1}))],'core/personas/ver','Personas');
op('POST','/api/v1/persons',profile,[],'core/personas/editar','Crear persona');op('PATCH','/api/v1/persons/{id}',profile,[],'core/personas/editar');op('DELETE','/api/v1/persons/{id}',null,[],'core/personas/eliminar');
for(const [resource,label] of [['users','Usuarios'],['roles','Roles']])op('GET','/api/v1/admin/'+resource,null,[query('page',int({default:1}))],'admin',label);
op('POST','/api/v1/admin/users',obj({person_id:int(),role_id:int(),email:str({format:'email'})}),[],'admin','Crear usuario');
op('PATCH','/api/v1/admin/users/{id}',obj({person_id:int(),role_id:int(),enabled:bool},[]),[],'admin');
op('POST','/api/v1/admin/users/{id}/invitation',null,[],'admin','Reenviar invitación');
op('POST','/api/v1/admin/roles',obj({name:str(),description:optional(str())}),[],'admin','Crear rol');
op('PATCH','/api/v1/admin/roles/{id}',obj({name:str(),description:optional(str()),permissions:arr(obj({module_id:str(),scope_key:str(),capability:en('ver','editar','eliminar')}))},[]),[],'admin');
op('DELETE','/api/v1/admin/roles/{id}',null,[],'admin');op('GET','/api/v1/admin/roles/{id}/permissions',null,[],'admin','Permisos');
for(const [resource,label] of [['core','Core'],['sar','SAR'],['modules','Módulos'],['audit','Auditoría'],['mail-outbox','Cola de correo']])op('GET','/api/v1/settings/'+resource,null,[],'admin',label);
op('PATCH','/api/v1/settings/modules/{module_id}',obj({enabled:bool}),[],'admin','Activar o desactivar');
op('POST','/api/v1/settings/mail-outbox/{id}/retry',null,[],'admin','Reintentar');
op('PUT','/api/v1/settings/sar',obj({settings:obj({limit_range:str(),limit_date:optional(date),current_invoice:str(),sar_text:str()}),version:str()}),[],'admin','Editar SAR');
op('PUT','/api/v1/settings/core/company.profile',obj({value:obj({name:str(),rtn:str(),email:str(),phone:str(),address:str()})}),[],'admin','Datos de empresa');
op('PUT','/api/v1/settings/core/smtp.host',obj({value:str()}),[],'admin','Servidor de correo');
op('PUT','/api/v1/settings/core/smtp.port',obj({value:int({enum:[465,587]})}),[],'admin','Puerto de correo');
op('PUT','/api/v1/settings/core/smtp.tls_mode',obj({value:en('implicit_tls','starttls_required')}),[],'admin','Seguridad del correo');
op('GET','/api/v1/settings/backups',null,[],'admin','Respaldos');
op('GET','/api/v1/procesos/dashboard',null,[],'procesos/consulta/ver','Resumen').module='procesos';
// Normalized labels and enum expansion are reviewed UI metadata, not arbitrary API entry.
for(const o of operations){
 for(const match of o.path.matchAll(/\{([^}]+)\}/g))if(!o.parameters.some(p=>p.in==='path'&&p.name===match[1]))o.parameters.push({in:'path',name:match[1],required:true,schema:str()});
 if(o.path.includes('/reportes/facturas'))for(const p of o.parameters)if(p.name==='status')p.schema.enum=['Borrador','Enviado','Pagado'];
}
function defaults(schema){if(!schema||typeof schema!=='object')return;for(const [key,p] of Object.entries(schema.properties||{})){
 if(p.default===undefined){if(['quantity','cantidad'].includes(key))p.default=p.type==='integer'?1:'1';if(/^(unit_price|cost_price|sale_price|purchase_price|discount_value|amount|taxable_amount|exempt_amount|debit|credit)$/.test(key))p.default='0.00';if(key==='valid_days')p.default=30;if(key==='credit_days')p.default=0;if(key==='tax_type')p.default='isv15';}
 if(key==='line_id')p.readOnly=true;defaults(p);
}if(schema.items)defaults(schema.items);}
for(const o of operations)defaults(o.body);
// Expand finite document/action choices into named controls; keep record identifiers contextual.
const expanded=[];
for(const operation of operations){
 let choices=[operation];
 for(const parameter of operation.parameters.filter(p=>p.in==='path'&&p.schema?.enum)){
  choices=choices.flatMap(o=>parameter.schema.enum.map(value=>({...o,path:o.path.replace('{'+parameter.name+'}',String(value)),title:o.title+' · '+({invoice:'Factura',delivery:'Nota de entrega',receipt:'Recibo',contabilizar:'Contabilizar',cancelar:'Cancelar',reversar:'Reversar'}[value]||value),parameters:o.parameters.filter(p=>p!==parameter)})));
 }
 for(const o of choices){if(o.path.endsWith('/cancelar')||o.path.endsWith('/reversar'))o.permission='contabilidad/libros/eliminar';if(o.path.includes('/facturas/')&&o.path.includes('/documents/'))o.permission='facturas/documentos/ver';expanded.push(o);}
}
const output={modules,operations:expanded};
const dest=path.join(root,'android/app/src/main/assets');fs.mkdirSync(dest,{recursive:true});
fs.writeFileSync(path.join(dest,'contracts.json'),JSON.stringify(output,null,2)+'\n');
console.log(`${modules.length} áreas, ${operations.length} operaciones nativas.`);
