import {test,expect} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {totp} from './helpers';

function expenseFixtureCredentials(){
 const source=readFileSync(join(process.cwd(),'e2e','zzzzzzzzz-expenses.spec.ts'),'utf8');
 const password=source.match(/getByLabel\('Contraseña'.*?\.fill\('([^']+)'\)/)?.[1];
 const secret=source.match(/totp\('([^']+)'\)/)?.[1];
 if(!password||!secret)throw new Error('Synthetic expense browser fixture unavailable');
 return {password,secret};
}

test('Gastos Unidad 2: referencia viva, comprobante en cuarentena y política de papelera',async({page})=>{
 test.setTimeout(90_000);
 const credentials=expenseFixtureCredentials();
 await page.goto('/login');
 await page.getByLabel('Correo',{exact:true}).fill('expense-links@example.test');
 await page.getByLabel('Contraseña',{exact:true}).fill(credentials.password);
 await page.getByRole('button',{name:'Ingresar',exact:true}).click();
 await page.getByLabel('Código de autenticación o recuperación').fill(totp(credentials.secret));
 await page.getByRole('button',{name:'Verificar',exact:true}).click();
 await page.getByRole('link',{name:'Gastos',exact:true}).click();

 await page.getByRole('button',{name:'Nuevo gasto',exact:true}).click();
 await page.getByLabel('Proveedor del gasto',{exact:true}).fill('Proveedor soporte sintético');
 await page.getByLabel('Documento del gasto',{exact:true}).fill('DOC-UNIT2-001');
 await page.getByLabel('Fecha del gasto',{exact:true}).fill('2026-10-12');
 await page.getByLabel('Categoría del gasto',{exact:true}).selectOption('services');
 await page.getByLabel('Monto gravado del gasto',{exact:true}).fill('100');
 await page.getByLabel('Monto exento del gasto',{exact:true}).fill('0');
 await page.getByRole('button',{name:'Calcular gasto',exact:true}).click();
 await expect(page.getByText('Total del gasto: HNL 115.00',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Registrar gasto',exact:true}).click();
 await expect(page.getByText('Versión de gasto: 1',{exact:true})).toBeVisible();

 const links=page.getByRole('region',{name:'Referencias y comprobantes del gasto'});
 await links.getByLabel('Proceso (ID)',{exact:true}).fill('1');
 await links.getByLabel('Motivo del cambio',{exact:true}).fill('Proceso fuente revisado');
 await links.getByRole('button',{name:'Guardar referencias',exact:true}).click();
 await expect(page.getByText('Versión de gasto: 2',{exact:true})).toBeVisible();
 await expect(links.getByText('Proceso: HC-TEST-001 · ID 1 · versión capturada 1',{exact:true})).toBeVisible();

 const receipt=Buffer.from('%PDF-1.4\nsynthetic expense receipt\n%%EOF');
 // File selection through the browser is unavailable while references/documents reload.
 await expect(links.getByLabel('Archivo comprobante',{exact:true})).toBeEnabled();
 await links.getByLabel('Archivo comprobante',{exact:true}).setInputFiles({name:'comprobante-gasto.pdf',mimeType:'application/pdf',buffer:receipt});
 await links.getByRole('button',{name:'Subir y vincular comprobante',exact:true}).click();
 await expect(page.getByText('Versión de gasto: 3',{exact:true})).toBeVisible();
 const documents=links.getByRole('region',{name:'Comprobantes del gasto'});
 await expect(documents.getByText(/comprobante-gasto\.pdf.*En cuarentena.*application\/pdf.*versión 2/)).toBeVisible();
 await expect(documents.getByRole('button',{name:'Vista previa',exact:true})).toHaveCount(0);
 await expect(documents.getByRole('button',{name:'Descargar',exact:true})).toHaveCount(0);

 await documents.getByRole('button',{name:'Eliminar comprobante comprobante-gasto.pdf',exact:true}).click();
 await expect(documents.getByText('0 comprobantes visibles · Página 1',{exact:true})).toBeVisible();
 await documents.getByLabel('Papelera de comprobantes',{exact:true}).check();
 await expect(documents.getByText(/comprobante-gasto\.pdf.*versión 3/)).toBeVisible();
 await documents.getByRole('button',{name:'Restaurar comprobante comprobante-gasto.pdf',exact:true}).click();
 await expect(documents.getByText('0 comprobantes visibles · Página 1',{exact:true})).toBeVisible();
 await documents.getByLabel('Papelera de comprobantes',{exact:true}).uncheck();
 await expect(documents.getByText(/comprobante-gasto\.pdf.*En cuarentena.*versión 4/)).toBeVisible();
 await expect(page.getByText('Versión de gasto: 3',{exact:true})).toBeVisible();

 await page.getByLabel('Motivo de cambio del gasto',{exact:true}).fill('Ocultar comprobante con el gasto');
 await page.getByRole('button',{name:'Enviar gasto a papelera',exact:true}).click();
 await expect(page.getByText('Versión de gasto: 4',{exact:true})).toBeVisible();
 await expect(documents.getByText('Los comprobantes están ocultos mientras el gasto permanezca en papelera.',{exact:true})).toBeVisible();
 await page.getByLabel('Motivo de cambio del gasto',{exact:true}).fill('Restaurar gasto y comprobante');
 await page.getByRole('button',{name:'Restaurar gasto',exact:true}).click();
 await expect(page.getByText('Versión de gasto: 5',{exact:true})).toBeVisible();
 await expect(documents.getByText(/comprobante-gasto\.pdf.*En cuarentena/)).toBeVisible();
});
