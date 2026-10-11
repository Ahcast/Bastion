import {test,expect,Page} from '@playwright/test';
import {totp} from './helpers';
async function login(page:Page,email:string){await page.goto('/login');await page.getByLabel('Correo',{exact:true}).fill(email);await page.getByLabel('Contraseña',{exact:true}).fill('synthetic browser password 42');await page.getByRole('button',{name:'Ingresar',exact:true}).click();await page.getByLabel('Código de autenticación o recuperación').fill(totp('JBSWY3DPEHPK3PXP'));await page.getByRole('button',{name:'Verificar',exact:true}).click();await page.getByRole('link',{name:'Cotizaciones',exact:true}).click()}
test('Cotizaciones: emisión aprobada, descarga real, recarga e historial tras revisión',async({page,browser})=>{
 test.setTimeout(90_000);
 const mark=(stage:string)=>console.log('[quote-emission-e2e] '+stage);
 const diagnostics=(p:Page)=>p.on('response',async(r)=>{if(r.status()>=400&&r.url().includes('/api/v1/')){let body='unavailable after navigation';try{body=await r.text()}catch{}console.log('Synthetic emission API failure',r.url(),r.status(),body)}});
 diagnostics(page);mark('start');
 await login(page,'emission-requester@example.test');await page.getByRole('button',{name:'Nueva cotización',exact:true}).click();await page.getByLabel('Nombre del cliente',{exact:true}).fill('Cliente emisión sintética');await page.getByRole('button',{name:'Agregar ítem',exact:true}).click();await page.getByLabel('Descripción 1',{exact:true}).fill('Equipo emitido');await page.getByLabel('Precio unitario 1',{exact:true}).fill('100');await page.getByRole('button',{name:'Guardar borrador',exact:true}).click();await expect(page.getByText('Versión 1 · Borrador',{exact:true})).toBeVisible();await page.getByRole('button',{name:'Pasar a Costeo',exact:true}).click();await page.getByRole('button',{name:'Pasar a Revisión',exact:true}).click();await expect(page.getByText('Versión 3 · Revisión',{exact:true})).toBeVisible();
 await expect(page.getByRole('button',{name:'Emitir PDF aprobado',exact:true})).toHaveCount(0);const expiry=new Date(Date.now()+86400000-6*3600000).toISOString().slice(0,16);await page.getByLabel('Vigencia de aprobación (hora de Honduras)',{exact:true}).fill(expiry);await page.getByRole('button',{name:'Solicitar aprobación',exact:true}).click();await expect(page.getByText(/Aprobación \d+ · Pendiente · Versión de cotización 4/)).toBeVisible();
 mark('request-approval:done');const other=await browser.newContext();const reviewer=await other.newPage();diagnostics(reviewer);try{
 await login(reviewer,'emission-reviewer@example.test');await reviewer.getByLabel('Buscar cotizaciones',{exact:true}).fill('Cliente emisión sintética');await reviewer.getByRole('button',{name:'Buscar',exact:true}).click();await expect(reviewer.getByText('1 cotizaciones · Página 1',{exact:true})).toBeVisible();await reviewer.getByRole('button',{name:/^Ver cotización COT-/}).click();await reviewer.getByRole('button',{name:'Consultar aprobaciones',exact:true}).click();await reviewer.getByLabel('Motivo de la solicitud o decisión',{exact:true}).fill('Revisión antes de emisión');await reviewer.getByRole('button',{name:'Aprobar cotización',exact:true}).click();await expect(reviewer.getByText('Versión 4 · Aprobada internamente',{exact:true})).toBeVisible();
 await reviewer.getByRole('button',{name:'Emitir PDF aprobado',exact:true}).click();await expect(reviewer.getByText('Versión 4 · Emitida · Disponible',{exact:false})).toBeVisible();await expect(reviewer.getByText(/Aprobación \d+ · Utilizada · Versión de cotización 4/)).toBeVisible();await expect(reviewer.getByRole('button',{name:'Emitir PDF aprobado',exact:true})).toHaveCount(0);
 mark('emit:done');const downloadEvent=reviewer.waitForEvent('download');await reviewer.getByRole('button',{name:'Descargar PDF versión 4',exact:true}).click();const download=await downloadEvent;expect(download.suggestedFilename()).toMatch(/-v4-emitida\.pdf$/);const stream=await download.createReadStream();const chunks:Buffer[]=[];for await(const chunk of stream!)chunks.push(Buffer.from(chunk));const bytes=Buffer.concat(chunks);expect(bytes.subarray(0,5).toString()).toBe('%PDF-');expect(bytes.length).toBeGreaterThan(1000);

 mark('download:done');
 // Commercial send exercises the real queue/worker with a network-free SMTP adapter.
 await page.reload();await page.getByRole('button',{name:/^Ver cotización COT-/}).first().click();
 await page.getByRole('button',{name:'Actualizar correo y ledger',exact:true}).click();
 await expect(page.getByRole('button',{name:'Actualizar correo y ledger',exact:true})).toBeEnabled();
 await page.getByLabel('Para',{exact:true}).fill('cliente@example.test');
 await expect(page.getByLabel('Para',{exact:true})).toHaveValue('cliente@example.test');
 await page.getByLabel('Motivo de autorización',{exact:true}).fill('Enviar PDF revisado');
 await expect(page.getByLabel('Para',{exact:true})).toHaveValue('cliente@example.test');
 await page.getByRole('button',{name:'Solicitar autorización de envío',exact:true}).click();
 await expect(page.getByText('Pendiente de aprobación',{exact:true})).toBeVisible();
 await reviewer.getByRole('button',{name:'Actualizar correo y ledger',exact:true}).click();
 await reviewer.getByLabel('Motivo de decisión o revocación',{exact:true}).fill('Destinatario y documento verificados');
 await reviewer.getByRole('button',{name:'Aprobar envío',exact:true}).click();
 mark('mail-approval:done');await reviewer.getByRole('button',{name:'Enviar / programar correo autorizado',exact:true}).click();
 await expect(async()=>{await reviewer.getByRole('button',{name:'Actualizar correo y ledger',exact:true}).click();await expect(reviewer.getByText('Aceptado por SMTP',{exact:true})).toBeVisible()}).toPass({timeout:15000});
 mark('smtp:done');await reviewer.reload();await reviewer.getByRole('button',{name:/^Ver cotización COT-/}).first().click();
 await expect(reviewer.getByText('Versión 5 · Enviada',{exact:true})).toBeVisible();
 await reviewer.getByLabel('Motivo del cambio comercial',{exact:true}).fill('Cliente confirma aceptación');
 await reviewer.getByRole('button',{name:'Pasar a Aceptada',exact:true}).click();
 await expect(reviewer.getByText('Versión 6 · Aceptada',{exact:true})).toBeVisible();
 await reviewer.getByLabel('Motivo del cambio comercial',{exact:true}).fill('Revisión solicitada por cliente');
 await reviewer.getByRole('button',{name:'Pasar a Borrador',exact:true}).click();
 await expect(reviewer.getByText('Versión 7 · Borrador',{exact:true})).toBeVisible();
 await reviewer.reload();await reviewer.getByRole('button',{name:/^Ver cotización COT-/}).filter({hasText:/COT-/}).first().click();await reviewer.getByRole('button',{name:'Consultar PDFs',exact:true}).click();await expect(reviewer.getByText('Versión 4 · Emitida · Disponible',{exact:false})).toBeVisible();await reviewer.getByLabel('Términos públicos',{exact:true}).fill('Nueva revisión posterior a emisión');await reviewer.getByRole('button',{name:'Guardar borrador',exact:true}).click();await expect(reviewer.getByText('Versión 8 · Borrador',{exact:true})).toBeVisible();await reviewer.getByRole('button',{name:'Consultar PDFs',exact:true}).click();await expect(reviewer.getByText('Versión 4 · Emitida · Disponible',{exact:false})).toBeVisible();
 mark('revision:done');}finally{await other.close()}
});
