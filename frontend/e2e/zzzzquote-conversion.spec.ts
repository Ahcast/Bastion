import {test,expect} from '@playwright/test';
import {totp} from './helpers';

test('Cotización aceptada a OC: conversión íntegra, recarga y vínculos cruzados',async({page})=>{
 test.setTimeout(90_000);
 await page.goto('/login');await page.getByLabel('Correo',{exact:true}).fill('conversion@example.test');await page.getByLabel('Contraseña',{exact:true}).fill('synthetic browser password 42');await page.getByRole('button',{name:'Ingresar',exact:true}).click();await page.getByLabel('Código de autenticación o recuperación').fill(totp('JBSWY3DPEHPK3PXP'));await page.getByRole('button',{name:'Verificar',exact:true}).click();
 await page.getByRole('link',{name:'Cotizaciones',exact:true}).click();await page.getByRole('button',{name:/^Ver cotización COT-/}).click();await expect(page.getByText('Versión 2 · Aceptada',{exact:true})).toBeVisible();
 await page.getByLabel('Número de OC del cliente',{exact:true}).fill('OC-CONVERSION-BROWSER');await page.getByLabel('Destino de la OC',{exact:true}).fill('Destino conversión');await page.getByLabel('Gastos adicionales de la OC',{exact:true}).fill('3.50');await page.getByLabel('Comentarios de la OC',{exact:true}).fill('Comentarios explícitos');
 // Forward the actual authenticated request unchanged; capture its server body
 // before returning it to the browser, whose protocol buffer can be discarded.
 let resolveConversion!:(value:{status:number;body:any})=>void;
 const response=new Promise<{status:number;body:any}>(resolve=>{resolveConversion=resolve});
 await page.route('**/convert-to-purchase-order',async route=>{const actual=await route.fetch();const body=await actual.json();await route.fulfill({response:actual});resolveConversion({status:actual.status(),body});});
 await page.getByRole('button',{name:'Confirmar conversión a OC',exact:true}).click();const result=await response;expect(result.status).toBe(200);const body=result.body;expect(body.purchase_order_id).toMatch(/^\d+$/);
 await expect(page.getByText('Versión 3 · Convertida a OC',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Confirmar conversión a OC',exact:true})).toHaveCount(0);
 await page.getByRole('link',{name:'Ver orden de compra',exact:true}).click();await expect(page.getByLabel('Número de orden',{exact:true})).toHaveValue('OC-CONVERSION-BROWSER');await expect(page.getByLabel('Nombre del cliente',{exact:true})).toHaveValue('Cliente conversión sintético');await expect(page.locator('select[name="currency"]')).toHaveValue('USD');await expect(page.getByLabel('Descripción',{exact:true})).toHaveValue('Equipo conversión sintético');await expect(page.getByLabel('Costo unitario',{exact:true})).toHaveValue('40');await expect(page.getByLabel('Descuento general',{exact:true})).toHaveValue('5');await expect(page.getByLabel('Comentarios de la orden',{exact:true})).toHaveValue('Comentarios explícitos');
 await expect(page.getByText('Total: USD 201.25',{exact:true})).toBeVisible();await expect(page.getByText('Inversión total: USD 83.50',{exact:true})).toBeVisible();await expect(page.getByText('Estado: Recibida',{exact:true})).toBeVisible();await expect(page.getByText('Estado guardado del ítem: Pendiente',{exact:true})).toBeVisible();
 await page.reload();await expect(page.getByLabel('Número de orden',{exact:true})).toHaveValue('OC-CONVERSION-BROWSER');await expect(page.getByText('Total: USD 201.25',{exact:true})).toBeVisible();
 await page.getByRole('link',{name:'Ver cotización de origen',exact:true}).click();await expect(page.getByText('Versión 3 · Convertida a OC',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Confirmar conversión a OC',exact:true})).toHaveCount(0);await expect(page.getByRole('link',{name:'Ver orden de compra',exact:true})).toBeVisible();
});
