import type { Access } from './types.js';
export interface CollaborationFile { id: number; scope: 'empresa' | 'persona'; id_persona: number | null; title: string; notes: string; filename: string; size_bytes: number; created_at: string; mime: string; trashed_at: string | null; purge_after: string | null }
export interface FilePolicy { max_file_mb: number; trash_retention_days: number; allowed_extensions: string[]; blocked_extensions: string[] }
export interface FilePage<T = CollaborationFile> { items: T[]; page: number; page_size: number; has_more: boolean }
export interface FileContent { filename: string; mime: string; content_base64: string }
export const MAX_FILE_BYTES = 100 * 1024 * 1024;
export function collaborationCan(access: Access, capability = 'ver') {
  return access.modules.some(m => m.module_id === 'colaboracion') && (access.is_system_administrator || access.permissions.some(p => p.module_id === 'colaboracion' && p.capability === capability));
}
export async function fileBase64(file: File, maxMb = 50): Promise<string> {
  if (!file.size || file.size > Math.min(MAX_FILE_BYTES, maxMb * 1024 * 1024)) throw new Error(`Selecciona un archivo no vacío de hasta ${maxMb} MB.`);
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error('No se pudo leer el archivo.'));
    reader.onload = () => typeof reader.result === 'string' ? resolve(reader.result.split(',')[1] ?? '') : reject(new Error('No se pudo leer el archivo.'));
    reader.readAsDataURL(file);
  });
}
export function contentBlob(content: FileContent, preview: boolean): Blob {
  if (content.content_base64.length > Math.ceil(MAX_FILE_BYTES / 3) * 4) throw new Error('El archivo recibido supera el límite.');
  const raw = atob(content.content_base64);
  if (raw.length > MAX_FILE_BYTES) throw new Error('El archivo recibido supera el límite.');
  const allowed = ['application/pdf', 'image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/bmp', 'text/plain'];
  if (preview && !allowed.includes(content.mime)) throw new Error('Este tipo de archivo solo admite descarga.');
  return new Blob([Uint8Array.from(raw, char => char.charCodeAt(0))], { type: preview ? content.mime : 'application/octet-stream' });
}
