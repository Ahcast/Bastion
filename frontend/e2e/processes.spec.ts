import {test,expect} from '@playwright/test';
import {totp} from './helpers';

test('Procesos: consulta, interés y nota sobreviven recarga con API real',async({page})=>{
 await page.goto('/login');
 await page.getByLabel('Correo',{exact:true}).fill('processes@example.test');
 await page.getByLabel('Contraseña',{exact:true}).fill('synthetic browser password 42');
 await page.getByRole('button',{name:'Ingresar',exact:true}).click();
 await page.getByLabel('Código de autenticación o recuperación').fill(totp('JBSWY3DPEHPK3PXP'));
 await page.getByRole('button',{name:'Verificar',exact:true}).click();
 await page.getByRole('link',{name:'Procesos',exact:true}).click();
 await page.getByRole('button',{name:'Ver proceso HC-TEST-001',exact:true}).click();
 await page.getByRole('button',{name:'Marcar interés',exact:true}).click();
 await expect(page.getByRole('button',{name:'Desmarcar interés',exact:true})).toBeVisible();
 await page.getByLabel('Nota personal',{exact:true}).fill('Nota sintética del navegador');
 const noteSaved=page.waitForResponse(response=>response.request().method()==='PATCH'&&new URL(response.url()).pathname==='/api/v1/processes/1/note');
 await page.getByRole('button',{name:'Guardar nota',exact:true}).click();
 const response=await noteSaved;
 expect(response.status()).toBe(200);
 await expect(page.getByRole('button',{name:'Guardar nota',exact:true})).toBeEnabled();
 await page.reload();
 await page.getByRole('button',{name:'Ver proceso HC-TEST-001',exact:true}).click();
 await expect(page.getByLabel('Nota personal',{exact:true})).toHaveValue('Nota sintética del navegador');
 await expect(page.getByRole('button',{name:'Desmarcar interés',exact:true})).toBeVisible();
 await page.getByLabel('Buscar en expediente y especificaciones').fill('sin-coincidencia-sintetica');
 await page.getByRole('button',{name:'Aplicar filtros',exact:true}).click();
 await expect(page.getByText('No hay procesos para estos filtros.')).toBeVisible();
});
