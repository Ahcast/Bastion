const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const root=path.resolve(__dirname,'../..'),asset=path.join(root,'android/app/src/main/assets/contracts.json');
const before=fs.readFileSync(asset,'utf8');require('./generate-contract.cjs');assert.equal(fs.readFileSync(asset,'utf8'),before,'Regenerate native contracts after changing API metadata');
const data=JSON.parse(before),routes=fs.readdirSync(path.join(root,'crates/api/src')).filter(f=>f.endsWith('.rs')).flatMap(f=>[...fs.readFileSync(path.join(root,'crates/api/src',f),'utf8').matchAll(/"(\/api\/v1\/[^"\n]*)"/g)].map(m=>m[1]));
function schema(s,location){if(!s)return;if(s.properties){for(const k of s.required||[])assert.ok(s.properties[k],`${location}: required field ${k} has no editor`);for(const [k,v] of Object.entries(s.properties))schema(v,location+'.'+k);}if(s.items)schema(s.items,location+'[]');}
for(const o of data.operations){assert.ok(routes.some(r=>new RegExp('^'+r.replace(/[.*+?^$()|[\]\\]/g,'\\$&').replace(/\{[^}]+\}/g,'[^/]+')+'$').test(o.path)),o.path+' is not a registered API route');schema(o.body,o.path);}
for(const module of data.modules)assert.ok(data.operations.some(o=>o.module===module.id),module.id);
assert.equal(data.modules.length,12);
for(const p of ['/api/v1/cotizaciones','/api/v1/facturas','/api/v1/ordenes-compra']){const body=data.operations.find(o=>o.path===p&&o.method==='POST').body;assert.ok((body.properties.items||body.properties.products).items.properties.description);}
console.log('PASS: native routes, required fields, document descriptions and reproducible contracts');
