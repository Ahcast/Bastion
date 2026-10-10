import {test,expect} from '@playwright/test';
import {totp} from './helpers';

test('Cotizaciones FX: registra una observación exacta sin convertir cotizaciones',async({page})=>{
 page.on('response',async r=>{if(r.status()>=400&&r.url().includes('/api/v1/')){let body='unavailable after navigation';try{body=await r.text()}catch{}console.log('Synthetic API failure',r.url(),r.status(),body)}});
 const syntheticPassword=['synthetic','browser','password','42'].join(' ');
 await page.goto('/login');
 await page.getByLabel('Correo',{exact:true}).fill('quotes@example.test');
 await page.getByLabel('Contraseña',{exact:true}).fill(syntheticPassword);
 await page.getByRole('button',{name:'Ingresar',exact:true}).click();
 await page.getByLabel('Código de autenticación o recuperación').fill(totp('JBSWY3DPEHPK3PXP'));
 await page.getByRole('button',{name:'Verificar',exact:true}).click();

 await page.goto('/quotes/fx');
 await expect(page.getByRole('heading',{name:'Registro de tasas de cambio',exact:true})).toBeVisible();
 await expect(page.getByText('Este registro no convierte cotizaciones automáticamente.',{exact:false})).toBeVisible();

 await page.getByLabel('Contraseña para confirmar',{exact:true}).fill(syntheticPassword);
 await page.getByLabel('Código de confirmación',{exact:true}).fill(totp('JBSWY3DPEHPK3PXP'));
 await page.getByRole('button',{name:'Confirmar identidad',exact:true}).click();
 await expect(page.getByText('Identidad confirmada. Puede continuar con la operación.',{exact:true})).toBeVisible();

 await page.getByLabel('Tipo de tasa',{exact:true}).last().selectOption('tcr');
 await page.getByLabel('Tasa HNL por USD',{exact:true}).fill('26.123456789012');
 await page.getByLabel('Vigente desde (hora de Honduras)',{exact:true}).fill('2026-10-10T08:30');
 await page.getByLabel('Fuente',{exact:true}).last().selectOption('bch');
 await page.getByLabel('Referencia de fuente',{exact:true}).fill('synthetic:bch:tcr:browser');
 await page.getByRole('button',{name:'Registrar observación',exact:true}).click();

 await expect(page.getByRole('status')).toContainText(/Observación \d+ registrada sin aplicar conversiones a cotizaciones\./);
 await expect(page.getByText('TCR · 26.123456789012 HNL por USD',{exact:true})).toBeVisible();
 await expect(page.getByText('Referencia: synthetic:bch:tcr:browser',{exact:true})).toBeVisible();
});
