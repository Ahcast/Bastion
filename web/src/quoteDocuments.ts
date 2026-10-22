export interface QuotePdf { filename: string; mime: 'application/pdf'; content_base64: string }
export interface DocumentReadiness { pdf: boolean; company: boolean; email: boolean }
export interface QuoteEmailInput { to: string; cc: string; bcc: string; subject: string; message: string; scheduled_at: string | null }
export interface QuoteEmailJob { id: number; delivery_status: 'pending' | 'delivering' | 'sent' | 'failed' | 'cancelled'; scheduled_at: string; last_error_code: string | null }
export function pdfBlob(pdf: QuotePdf): Blob {
  if (pdf.mime !== 'application/pdf' || pdf.content_base64.length > 12 * 1024 * 1024) throw new Error('El PDF recibido no es válido.');
  const raw = atob(pdf.content_base64);
  if (!raw.startsWith('%PDF-')) throw new Error('El PDF recibido no es válido.');
  return new Blob([Uint8Array.from(raw, char => char.charCodeAt(0))], { type: 'application/pdf' });
}
export function downloadPdf(pdf: QuotePdf) {
  const url = URL.createObjectURL(pdfBlob(pdf)), link = document.createElement('a');
  link.href = url; link.download = pdf.filename; document.body.append(link); link.click(); link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
export function hondurasSchedule(value: string): string | null {
  if (!value) return null;
  const date = new Date(value + '-06:00');
  if (!Number.isFinite(date.getTime())) throw new Error('La fecha de envío no es válida.');
  return date.toISOString();
}
export function emailFailure(code: string | null): string {
  if (code === 'quote_changed') return 'La cotización cambió antes del envío. Abre su versión actual para preparar otro correo.';
  if (code === 'authorization_changed') return 'El envío se detuvo porque cambiaron los permisos o la disponibilidad de la cotización.';
  if (code === 'configuration_invalid') return 'Revisa la configuración de empresa y las credenciales SMTP en Ajustes.';
  return 'No se pudo confirmar la entrega. Revisa tu correo enviado antes de preparar otro envío para evitar duplicados.';
}
