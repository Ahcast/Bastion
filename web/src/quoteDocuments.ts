import { fileBlob } from './quoteAttachments.js';
import type { QuoteFile } from './quoteAttachments.js';
export interface QuotePdf { filename: string; mime: 'application/pdf'; content_base64: string }
export interface DocumentReadiness { pdf: boolean; company: boolean; email: boolean; attachments?: boolean }
export interface QuoteEmailInput { to: string; cc: string; bcc: string; subject: string; message: string; scheduled_at: string | null }
export interface QuoteEmailJob { id: number; delivery_status: 'pending' | 'delivering' | 'sent' | 'failed' | 'cancelled'; scheduled_at: string; last_error_code: string | null }
export function pdfBlob(pdf: QuotePdf): Blob {
  if (pdf.mime !== 'application/pdf' || pdf.content_base64.length > 12 * 1024 * 1024) throw new Error('El PDF recibido no es válido.');
  const raw = atob(pdf.content_base64);
  if (!raw.startsWith('%PDF-')) throw new Error('El PDF recibido no es válido.');
  return new Blob([Uint8Array.from(raw, char => char.charCodeAt(0))], { type: 'application/pdf' });
}
export function reservePdfViewer() {
  // Reserve the tab in the click event, before asynchronous generation, to avoid popup blocking.
  const viewer = window.open('about:blank', '_blank');
  if (!viewer) throw new Error('El navegador bloqueó la pestaña del PDF. Permite abrirla y vuelve a intentarlo.');
  viewer.opener = null; viewer.document.title = 'Preparando PDF';
  if (viewer.document.body) viewer.document.body.textContent = 'Preparando PDF…';
  let url: string | null = null, poll: ReturnType<typeof setInterval> | undefined, expiry: ReturnType<typeof setTimeout> | undefined;
  const release = () => { if (poll) clearInterval(poll); if (expiry) clearTimeout(expiry); if (url) { URL.revokeObjectURL(url); url = null; } };
  return {
    show(file: QuoteFile) {
      if (file.mime !== 'application/pdf') throw new Error('El archivo recibido no es un PDF.');
      if (viewer.closed) return;
      // Do not replace a tab that the user has navigated elsewhere while generation ran.
      try { if (viewer.location.href !== 'about:blank') return; } catch { return; }
      const blob = fileBlob(file);
      url = URL.createObjectURL(blob); viewer.document.title = file.filename; viewer.location.replace(url);
      poll = setInterval(() => { if (viewer.closed) release(); }, 1000);
      expiry = setTimeout(release, 60 * 60 * 1000);
    },
    close() {
      try { if (!viewer.closed && (viewer.location.href === 'about:blank' || viewer.location.href === url)) viewer.close(); } catch { /* User navigated the tab away. */ }
      release();
    },
  };
}
export function viewPdf(pdf: QuotePdf) {
  const viewer = reservePdfViewer();
  try { viewer.show(pdf); } catch (reason) { viewer.close(); throw reason; }
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
