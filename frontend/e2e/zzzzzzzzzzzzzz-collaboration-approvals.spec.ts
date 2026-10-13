import {expect,test,Page} from '@playwright/test';
import {totp} from './helpers';

async function login(page:Page,email:string) {
 await page.goto('/login');
 await page.getByLabel('Correo',{exact:true}).fill(email);
 await page.getByLabel('Contraseña',{exact:true}).fill('synthetic browser password 42');
 await page.getByRole('button',{name:'Ingresar',exact:true}).click();
 await page.getByLabel('Código de autenticación o recuperación').fill(totp('JBSWY3DPEHPK3PXP'));
 await page.getByRole('button',{name:'Verificar',exact:true}).click();
 await expect(page.getByRole('link',{name:'Colaboración',exact:true})).toBeVisible();
}

test('Colaboración: bandeja real abre la cotización propietaria y refleja su decisión',async({page,browser})=>{
 await login(page,'collaboration-inbox-requester@example.test');
 await page.getByRole('link',{name:'Cotizaciones',exact:true}).click();
 await page.getByRole('button',{name:'Nueva cotización',exact:true}).click();
 const customer='Cliente bandeja Colaboración sintética';
 await page.getByLabel('Nombre del cliente',{exact:true}).fill(customer);
 await page.getByRole('button',{name:'Agregar ítem',exact:true}).click();
 await page.getByLabel('Descripción 1',{exact:true}).fill('Equipo de prueba de bandeja');
 await page.getByLabel('Precio unitario 1',{exact:true}).fill('100');
 await page.getByRole('button',{name:'Guardar borrador',exact:true}).click();
 await expect(page.getByText('Versión 1 · Borrador',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Pasar a Costeo',exact:true}).click();
 await page.getByRole('button',{name:'Pasar a Revisión',exact:true}).click();
 await expect(page.getByText('Versión 3 · Revisión',{exact:true})).toBeVisible();
 await page.getByLabel('Vigencia de aprobación (hora de Honduras)',{exact:true}).fill(new Date(Date.now()+86400000-6*3600000).toISOString().slice(0,16));
 await page.getByLabel('Motivo de la solicitud o decisión',{exact:true}).fill('Revisión desde bandeja');
 await page.getByRole('button',{name:'Solicitar aprobación',exact:true}).click();
 await expect(page.getByText(/Aprobación \d+ · Pendiente · Versión de cotización 4/)).toBeVisible();
 const source=await page.request.get('http://localhost:8080/api/v1/quotes?search='+encodeURIComponent(customer)+'&page=1&per_page=25');
 expect(source.ok()).toBeTruthy();
 const quote=(await source.json()).data[0];expect(quote?.id).toBeTruthy();
 await page.goto('/collaboration/integrations');
 await expect(page.locator('a[href="/quotes?id='+quote.id+'"]')).toHaveCount(0);
 const context=await browser.newContext();const reviewer=await context.newPage();
 try {
  await login(reviewer,'collaboration-inbox-reviewer@example.test');
  await reviewer.goto('http://localhost:4200/collaboration/integrations');
  const ownerLink=reviewer.locator('a[href="/quotes?id='+quote.id+'"]');
  await expect(ownerLink).toBeVisible();
  await expect(reviewer.getByRole('button',{name:'Aprobar cotización',exact:true})).toHaveCount(0);
  await ownerLink.click();
  await expect(reviewer.getByText('Versión 3 · Revisión',{exact:true})).toBeVisible();
  await reviewer.getByRole('button',{name:'Consultar aprobaciones',exact:true}).click();
  await reviewer.getByLabel('Motivo de la solicitud o decisión',{exact:true}).fill('Verificada desde propietario');
  await reviewer.getByRole('button',{name:'Aprobar cotización',exact:true}).click();
  await expect(reviewer.getByText('Versión 4 · Aprobada internamente',{exact:true})).toBeVisible();
  await reviewer.goto('http://localhost:4200/collaboration/integrations');
  await expect(reviewer.getByRole('heading',{name:'Integraciones y aprobaciones'})).toBeVisible();
  const inbox=await reviewer.request.get('http://localhost:8080/api/v1/collaboration/approvals?page=1&per_page=100');
  expect(inbox.ok()).toBeTruthy();
  expect((await inbox.json()).data.some((item:{entity_id:string})=>item.entity_id===quote.id)).toBe(false);
  await expect(reviewer.locator('a[href="/quotes?id='+quote.id+'"]')).toHaveCount(0);
 } finally {await context.close()}
});
