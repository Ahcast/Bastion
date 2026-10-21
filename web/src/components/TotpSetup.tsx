import { useMemo } from 'react';
import qrcode from 'qrcode-generator';
import type { Enrollment } from '../types.js';
export function qrMatrix(uri: string): boolean[][] {
  const qr = qrcode(0, 'M'); qr.addData(uri); qr.make();
  return Array.from({ length: qr.getModuleCount() }, (_, y) => Array.from({ length: qr.getModuleCount() }, (_, x) => qr.isDark(y, x)));
}
export default function TotpSetup({ setup }: { setup: Enrollment }) {
  const matrix = useMemo(() => { try { return qrMatrix(setup.otpauth_uri); } catch { return null; } }, [setup.otpauth_uri]);
  const size = (matrix?.length ?? 0) + 8;
  return <div className="totp-setup"><p>Escanea el QR desde tu app autenticadora para agregar la cuenta Bastion.</p>{matrix ? <div className="totp-qr"><svg role="img" aria-label="Código QR de configuración TOTP" viewBox={`0 0 ${size} ${size}`} xmlns="http://www.w3.org/2000/svg"><rect width={size} height={size} fill="white" /><path fill="black" d={matrix.flatMap((row, y) => row.flatMap((dark, x) => dark ? [`M${x + 4},${y + 4}h1v1h-1z`] : [])).join('')} /></svg></div> : <p role="alert">No se pudo generar el QR. Usa la clave manual.</p>}<div className="totp-manual"><strong>¿No puedes escanearlo?</strong><p>Agrega una cuenta por clave manual y selecciona códigos basados en tiempo (TOTP).</p><code>{setup.totp_secret}</code></div><details className="totp-manual"><summary>Ver URI de configuración</summary><code>{setup.otpauth_uri}</code></details></div>;
}
