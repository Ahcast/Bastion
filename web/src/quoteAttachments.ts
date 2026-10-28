import type { QuotePdf } from './quoteDocuments.js';
export const maxAttachmentBytes = 20 * 1024 * 1024;
export interface QuoteFile { filename: string; mime: string; content_base64: string }
export interface QuoteAttachment { id: string; filename: string; title: string; mime: string; byte_size: number; created_at: string }
export interface AttachmentPage { items: QuoteAttachment[]; page: number; has_more: boolean; quote_version: number }
export interface AttachmentWrite { attachment: QuoteAttachment; quote_version: number }
export function fileBlob(file: QuoteFile, limit = maxAttachmentBytes): Blob {
  if (file.content_base64.length > Math.ceil(limit / 3) * 4) throw new Error('El archivo supera el límite permitido.');
  const raw = atob(file.content_base64);
  if (raw.length > limit) throw new Error('El archivo supera el límite permitido.');
  if (file.mime === 'application/pdf' && !raw.startsWith('%PDF-')) throw new Error('El PDF recibido no es válido.');
  return new Blob([Uint8Array.from(raw, char => char.charCodeAt(0))], { type: file.mime === 'application/pdf' ? 'application/pdf' : 'application/octet-stream' });
}
export function downloadFile(file: QuoteFile) {
  const url = URL.createObjectURL(fileBlob(file)), link = document.createElement('a');
  link.href = url; link.download = file.filename; document.body.append(link); link.click(); link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
export async function encodeAttachment(file: File): Promise<string> {
  if (!file.size || file.size > maxAttachmentBytes) throw new Error('Cada archivo debe tener contenido y medir hasta 20 MiB.');
  const bytes = new Uint8Array(await file.arrayBuffer()); let text = '';
  for (let index = 0; index < bytes.length; index += 32768) text += String.fromCharCode(...bytes.subarray(index, index + 32768));
  return btoa(text);
}
export function fileSize(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} KiB` : `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}
export function emailAttachmentError(page: AttachmentPage, pdf: QuotePdf): string {
  if (page.has_more || page.items.length > 10) return 'El correo admite hasta 10 archivos adicionales. Retira los restantes antes de enviar.';
  const bytes = page.items.reduce((sum, file) => sum + file.byte_size, 0) + atob(pdf.content_base64).length;
  return bytes > maxAttachmentBytes ? 'El PDF y los archivos del correo superan el límite conjunto de 20 MiB.' : '';
}
