import { contentBlob } from './collaboration.js';
import type { FileContent } from './collaboration.js';
export type AttachmentSection = 'legal' | 'tecnica';
export interface TenderAttachment { id: string; section: AttachmentSection; filename: string; mime: string; byte_size: number; sha256: string; source_id: number | null; source_sha256: string | null; created_at: string }
export interface TenderAttachments { items: TenderAttachment[]; expedient_version: number; retained_bytes: number }
export const maxTenderFileBytes = 20_000_000;
export const tenderFileAccept = '.pdf,.png,.jpg,.jpeg,.gif,.webp,.bmp,.tif,.tiff';
export const tenderMimes = ['application/pdf','image/png','image/jpeg','image/gif','image/webp','image/bmp','image/x-ms-bmp','image/tiff'];
export function tenderSize(bytes: number) { return bytes < 1000 ? `${bytes} B` : bytes < 1_000_000 ? `${(bytes / 1000).toFixed(1)} kB` : `${(bytes / 1_000_000).toFixed(2)} MB`; }
export function validateTenderFile(file: Pick<File, 'name' | 'size'>) {
  if (!file.size || file.size > maxTenderFileBytes) throw new Error('Cada archivo debe tener contenido y medir hasta 20 MB.');
  if (!/\.(pdf|png|jpe?g|gif|webp|bmp|tiff?)$/i.test(file.name)) throw new Error('Selecciona únicamente PDF o imágenes PNG, JPEG, GIF, WebP, BMP o TIFF.');
}
export function tenderContentBlob(content: FileContent, preview: boolean) {
  if (content.content_base64.length > Math.ceil(maxTenderFileBytes / 3) * 4 || atob(content.content_base64).length > maxTenderFileBytes) throw new Error('El archivo recibido supera 20 MB.');
  return contentBlob(content.mime === 'image/x-ms-bmp' ? { ...content,mime:'image/bmp' } : content, preview);
}
