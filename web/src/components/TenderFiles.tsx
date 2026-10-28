import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Dialog, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { collaborationCan } from '../collaboration.js';
import type { CollaborationFile, FileContent, FilePage } from '../collaboration.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { reservePdfViewer } from '../quoteDocuments.js';
import { DocumentPagination } from './DocumentWorkspace.js';
import { encodeTenderFile, maxTenderFileBytes, tenderContentBlob, tenderFileAccept, tenderMimes, tenderSize, validateTenderFile } from '../tenderAttachments.js';
import type { AttachmentSection, TenderAttachment, TenderAttachments } from '../tenderAttachments.js';
import type { Expedient } from '../tenders.js';

export default function TenderFiles({ record, section, changed, start }: { record: Expedient; section: AttachmentSection; changed: (value: Expedient) => void; start: (action: () => void) => void }) {
  const { api, access } = useCore();
  const [upload, setUpload] = useState(false), [library, setLibrary] = useState(false), [removing, setRemoving] = useState<TenderAttachment | null>(null);
  const [error, setError] = useState(''), [busy, setBusy] = useState(false), [preview, setPreview] = useState<{ url: string; title: string } | null>(null);
  const result = useLoad(signal => api.request<TenderAttachments>(`/api/v1/licitaciones/${record.id}/archivos`, { signal }), [record.id, record.version]);
  const editable = record.state === 'borrador' && moduleCan(access,'licitaciones','expedientes','editar');
  const canRemove = record.state === 'borrador' && moduleCan(access,'licitaciones','expedientes','eliminar');
  useEffect(() => () => { if (preview) URL.revokeObjectURL(preview.url); }, [preview]);
  const read = async (file: TenderAttachment, show: boolean) => {
    let viewer: ReturnType<typeof reservePdfViewer> | undefined;
    try { if (show && file.mime === 'application/pdf') viewer = reservePdfViewer(maxTenderFileBytes); } catch (reason) { setError(message(reason)); return; }
    setBusy(true); setError('');
    try {
      const content = await api.request<FileContent>(`/api/v1/licitaciones/${record.id}/archivos/${file.id}/${show ? 'preview' : 'download'}`);
      if (viewer) { tenderContentBlob(content,true); viewer.show(content); }
      else {
        const url = URL.createObjectURL(tenderContentBlob(content,show));
        if (show) setPreview({ url, title: file.filename });
        else { const link = document.createElement('a'); link.href = url; link.download = file.filename; document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url),60_000); }
      }
    } catch (reason) { viewer?.close(); setError(message(reason)); } finally { setBusy(false); }
  };
  return <div className="tender-files"><div className="document-section-heading"><h3>Archivos del expediente</h3>{editable && <div className="actions"><button disabled={busy} onClick={() => start(() => setUpload(true))}>Subir desde mi PC</button>{collaborationCan(access) && <button disabled={busy} onClick={() => start(() => setLibrary(true))}>Seleccionar de Colaboración</button>}</div>}</div>
    <p className="document-note">PDF e imágenes, hasta 100 MB por archivo y 1 GB por expediente. Los archivos de Colaboración se referencian al original.</p><Notice>{error || result.error}</Notice>
    {result.data ? <><Table columns={['Archivo','Tamaño','Origen','Acciones']} rows={result.data.items.filter(f => f.section === section).map(file => [file.filename,tenderSize(file.byte_size),file.is_reference ? 'Referencia de Colaboración' : file.source_id ? 'Copia anterior' : 'PC',<div className="actions">{file.available && file.mime !== 'image/tiff' && <button disabled={busy} onClick={() => { void read(file,true); }}>Ver</button>}{file.available && <button disabled={busy} onClick={() => { void read(file,false); }}>Descargar</button>}{canRemove && <button className="danger" disabled={busy} onClick={() => start(() => setRemoving(file))}>Quitar archivo</button>}</div>])} /><p className="document-note">Espacio retenido en el expediente: {tenderSize(result.data.retained_bytes)} de 1 GB.</p></> : !result.error && <Loading />}
    {upload && <UploadFiles record={record} section={section} changed={changed} close={() => setUpload(false)} />}
    {library && <LibraryFiles record={record} section={section} changed={value => { changed(value); setLibrary(false); }} close={() => setLibrary(false)} />}
    {removing && <RemoveFile record={record} file={removing} changed={value => { changed(value); setRemoving(null); }} close={() => setRemoving(null)} />}
    {preview && <Dialog title={preview.title} close={() => setPreview(null)}><img className="collaboration-image" src={preview.url} alt={preview.title} /></Dialog>}
  </div>;
}
function UploadFiles({ record, section, changed, close }: { record: Expedient; section: AttachmentSection; changed: (value: Expedient) => void; close: () => void }) {
  const { api } = useCore(), { navigate } = useNavigationGuard();
  const [entries,setEntries] = useState<{ file: File; key: string; done: boolean }[]>([]), [busy,setBusy] = useState(false), [error,setError] = useState(''), [conflict,setConflict] = useState(false);
  const version = useRef(record.version);
  useUnsavedChanges(entries.some(e => !e.done),busy);
  return <Dialog title="Subir archivos desde mi PC" close={() => { if (!busy) void navigate(close); }}><form className="stack" onSubmit={async event => {
    event.preventDefault(); if (busy) return; setBusy(true); setError(''); setConflict(false);
    try {
      for (const entry of entries.filter(e => !e.done)) {
        validateTenderFile(entry.file);
        const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/archivos`, { method:'POST', ifMatch:version.current, idempotencyKey:entry.key, body:{ section,filename:entry.file.name,content_base64:await encodeTenderFile(entry.file) } });
        version.current = updated.version; changed(updated); setEntries(current => current.map(e => e.key === entry.key ? { ...e,done:true } : e));
      }
    } catch (reason) { setError(message(reason)); setConflict(reason instanceof ApiError && reason.status === 412); } finally { setBusy(false); }
  }}><Notice>{error}</Notice>{conflict && <button type="button" disabled={busy} onClick={async () => { setBusy(true); try { const current = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}`); version.current = current.version; changed(current); setConflict(false); setError('Puedes reintentar los archivos pendientes.'); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>Actualizar versión para reintentar</button>}
    <fieldset className="document-fields" disabled={busy}><label className="document-wide">PDF o imágenes<input type="file" multiple accept={tenderFileAccept} onChange={event => { try { const files = Array.from(event.target.files ?? []); files.forEach(validateTenderFile); setEntries(current => [...current,...files.map(file => ({ file,key:crypto.randomUUID(),done:false }))]); setError(''); } catch (reason) { setError(message(reason)); } event.target.value = ''; }} /></label>
      <ul className="document-wide tender-upload-list">{entries.map(e => <li key={e.key}><span>{e.file.name}<small>{tenderSize(e.file.size)}</small></span><span>{e.done ? 'Guardado' : busy ? 'En cola / subiendo' : 'Pendiente'}</span></li>)}</ul>
      <p className="document-wide document-note">Se incorporan en orden. Si una subida falla, los archivos ya guardados permanecen y puedes reintentar los pendientes.</p>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={!entries.some(e => !e.done)}>{busy ? 'Subiendo…' : 'Subir archivos'}</button></div>
    </fieldset></form></Dialog>;
}
function LibraryFiles({ record,section,changed,close }: { record: Expedient; section: AttachmentSection; changed: (value: Expedient) => void; close: () => void }) {
  const { api } = useCore();
  const [page,setPage] = useState(1), [search,setSearch] = useState(''), [query,setQuery] = useState(''), [busy,setBusy] = useState(false), [error,setError] = useState('');
  const attempt = useRef<{ id: number; key: string } | null>(null);
  useUnsavedChanges(false,busy);
  const result = useLoad(signal => api.request<FilePage<CollaborationFile>>(`/api/v1/colaboracion?trash=false&search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query,page]);
  return <Dialog title="Seleccionar archivo de Colaboración" close={() => { if (!busy) close(); }}><p className="document-note">Sólo se muestran PDF e imágenes de las carpetas que puedes leer. Se crea una referencia al original en Colaboración; su disponibilidad y permisos se comprueban al abrirlo.</p><form className="row" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar archivo<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button disabled={busy}>Buscar</button></form><Notice>{error || result.error}</Notice>
    {result.data ? <><Table columns={['Archivo','Tamaño','Acción']} rows={result.data.items.filter(f => tenderMimes.includes(f.mime)).map(file => [file.filename,tenderSize(file.size_bytes),<button disabled={busy || file.size_bytes > maxTenderFileBytes} title={file.size_bytes > maxTenderFileBytes ? 'Supera 100 MB' : undefined} onClick={async () => { if (busy) return; setBusy(true); setError(''); if (attempt.current?.id !== file.id) attempt.current = { id:file.id,key:crypto.randomUUID() }; try { changed(await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/archivos/desde-colaboracion`, { method:'POST', ifMatch:record.version, idempotencyKey:attempt.current.key, body:{ section,source_id:file.id } })); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>Incorporar archivo</button>])} /><DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} busy={busy} /></> : !result.error && <Loading />}
  </Dialog>;
}
function RemoveFile({ record,file,changed,close }: { record: Expedient; file: TenderAttachment; changed: (value: Expedient) => void; close: () => void }) {
  const { api } = useCore();const [busy,setBusy] = useState(false), [error,setError] = useState('');
  useUnsavedChanges(false,busy);
  return <Dialog title="Quitar archivo del expediente" close={() => { if (!busy) close(); }}><Notice>{error}</Notice><p>Se quitará «{file.filename}» del expediente. El original de Colaboración se conserva.</p><button className="danger" disabled={busy} onClick={async () => { setBusy(true); setError(''); try { changed(await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/archivos/${file.id}`, { method:'DELETE',ifMatch:record.version,body:{confirm_id:file.id} })); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>Confirmar quitar archivo</button></Dialog>;
}
