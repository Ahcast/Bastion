import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { message } from '../api.js';
import { moduleCan } from '../types.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { DocumentSection, DocumentPagination } from '../components/DocumentWorkspace.js';
import { sectionLabels } from '../tenders.js';
import type { FormDefinition, FormSection, PublishedForm, TenderPage } from '../tenders.js';
import type { CollaborationFile, FilePage } from '../collaboration.js';
import { fileBase64 } from '../collaboration.js';

export default function FormLibrary({ settings = false, installedRows = [] }: { settings?: boolean; installedRows?: ReactNode[][] }) {
  const { api, access, notify } = useCore();
  const [page, setPage] = useState(1), [editing, setEditing] = useState<PublishedForm | 'new' | null>(null);
  const [removing, setRemoving] = useState<PublishedForm | null>(null), [removingAll, setRemovingAll] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const allowed = moduleCan(access, 'colaboracion', 'empresa', 'ver');
  const result = useLoad(signal => allowed ? api.request<TenderPage<PublishedForm>>(`/api/v1/colaboracion/formularios?page=${page}`, { signal }) : Promise.resolve(null), [page, allowed]);
  if (!allowed && !settings) return <Notice>No tienes lectura sobre la biblioteca de empresa.</Notice>;
  return <div className="document-workspace"><Heading title={settings ? 'Archivos Typst' : 'Biblioteca de formularios'} description="Las variables y sus valores se definen en cada expediente." action={allowed && access.is_system_administrator && <div className="actions">{settings && <button className="danger" onClick={() => setRemovingAll(true)}>Eliminar todos los archivos legales y técnicos</button>}<button className="primary" onClick={() => setEditing('new')}>{settings ? 'Agregar archivo Typst' : 'Nuevo formulario'}</button></div>} />
    <DocumentSection title={settings ? 'Archivos disponibles' : 'Formularios publicados'} metadata={result.data ? `${result.data.items.length} de biblioteca en esta página` : undefined} list>
    <Notice>{error || result.error}</Notice>
    {result.data || !allowed || result.error ? <><Table columns={['Archivo', 'Sección', 'Estado / revisión', 'Acción']} rows={[...installedRows, ...(result.data?.items ?? []).map(form => [form.definition.title, sectionLabels[form.definition.section], form.revision, access.is_system_administrator ? <div className="actions"><button onClick={() => setEditing(form)}>Nueva revisión</button><button className="danger" onClick={() => setRemoving(form)}>{settings ? 'Eliminar archivo' : 'Quitar del catálogo'}</button></div> : 'Publicado'])]} />{result.data && <DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} />}</> : <Loading />}
    <p className="document-note">Selecciona un archivo .typ de Colaboración o súbelo desde tu PC. Las variables se crean en cada expediente.</p>
    </DocumentSection>
    {editing && <SchemaEditor initial={editing === 'new' ? null : editing} close={() => setEditing(null)} saved={() => { setEditing(null); result.reload(); notify('Esquema aprobado y publicado.'); }} />}
    {removing && <Dialog title={settings ? 'Eliminar archivo Typst' : 'Quitar formulario del catálogo'} close={() => { if (!busy) setRemoving(null); }}><p>{settings ? `«${removing.definition.title}» se eliminará del catálogo y de todos los expedientes que lo usan. Sus originales de Colaboración se programarán para eliminación permanente, excepto los archivos económicos y de soporte protegidos. Otros formularios que compartan los originales eliminados también dejarán de estar disponibles.` : `«${removing.definition.title}» dejará de estar disponible para nuevos expedientes. Los expedientes existentes conservarán su formulario.`}</p><Notice>{error}</Notice><div className="actions"><button disabled={busy} onClick={() => setRemoving(null)}>Cancelar</button><button className="danger" disabled={busy} onClick={async () => { setBusy(true); setError(''); try { await api.request(settings ? `/api/v1/licitaciones/plantillas/${removing.id}` : `/api/v1/colaboracion/formularios/${removing.id}`, { method:'DELETE', body:{expected_revision:removing.revision} }); setRemoving(null); result.reload(); notify(settings ? 'Formulario eliminado; se conservan los originales protegidos.' : 'Formulario retirado del catálogo.'); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>{settings ? 'Confirmar eliminación' : 'Quitar formulario'}</button></div></Dialog>}
    {removingAll && <Dialog title="Eliminar archivos legales y técnicos" close={() => { if (!busy) setRemovingAll(false); }}><p>Se eliminarán todos los Typst legales y técnicos del catálogo y de los expedientes que los usan. Sus originales de Colaboración se programarán para eliminación permanente. Se conservarán los archivos económicos y técnicos de configuración, ítems, lotes, variables y funciones.</p><Notice>{error}</Notice><div className="actions"><button disabled={busy} onClick={() => setRemovingAll(false)}>Cancelar</button><button className="danger" disabled={busy} onClick={async () => { setBusy(true); setError(''); try { const resultValue = await api.request<{deleted:number}>('/api/v1/licitaciones/plantillas', {method:'DELETE', body:{confirm:true}}); setRemovingAll(false); result.reload(); notify(`${resultValue.deleted} archivos retirados; originales programados para eliminación.`); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>Eliminar archivos</button></div></Dialog>}
  </div>;
}

function SchemaEditor({ initial, close, saved }: { initial: PublishedForm | null; close: () => void; saved: () => void }) {
  const { api } = useCore(), { navigate } = useNavigationGuard();
  const [definition, setDefinition] = useState<FormDefinition>(() => initial ? structuredClone(initial.definition) : { title: '', section: 'legal', variables: [] });
  const baseline = useRef(JSON.stringify(definition));
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [confirmed, setConfirmed] = useState(false);
  const [templatePage, setTemplatePage] = useState(1);
  const [uploading, setUploading] = useState(false);
  const uploadAttempt = useRef<{ file: File; key: string } | null>(null);
  const templates = useLoad(signal => api.request<FilePage<CollaborationFile>>(`/api/v1/colaboracion?trash=false&page=${templatePage}&page_size=25`, { signal }), [templatePage]);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  useUnsavedChanges(JSON.stringify(definition) !== baseline.current, busy || uploading);
  return <Dialog title={initial ? `Nueva revisión de ${initial.definition.title}` : 'Nuevo formulario'} close={() => { if (!busy) void navigate(close); }}>
    <form className="stack" onSubmit={async event => {
      event.preventDefault(); if (busy || uploading || !confirmed) return;
      if (!definition.template_source_id) { setError('Selecciona o sube una plantilla .typ.'); return; }
      const body = { form_id: initial?.id ?? null, expected_revision: initial?.revision ?? null, definition };
      const serialized = JSON.stringify(body);
      if (attempt.current?.body !== serialized) attempt.current = { body: serialized, key: crypto.randomUUID() };
      setBusy(true); setError('');
      try { await api.request('/api/v1/colaboracion/formularios', { method: 'POST', body, idempotencyKey: attempt.current.key }); baseline.current = JSON.stringify(definition); saved(); }
      catch (reason) { setError(message(reason)); } finally { setBusy(false); }
    }}><Notice>{error}</Notice><fieldset disabled={busy} className="document-fields">
      <label>Nombre del formulario<input value={definition.title} required maxLength={255} onChange={e => { setConfirmed(false); setDefinition(d => ({ ...d, title: e.target.value })); }} /></label>
      <label>Bloque<select value={definition.section} onChange={e => { setConfirmed(false); setDefinition(d => ({ ...d, section: e.target.value as FormSection })); }}>{Object.entries(sectionLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label>
      <div className="document-wide"><h3>Plantilla Typst</h3><label>Subir archivo .typ desde mi PC<input type="file" accept=".typ,text/plain" disabled={uploading} onChange={async event => { const file = event.currentTarget.files?.[0]; event.currentTarget.value = ''; if (!file) return; setUploading(true); setError(''); try { if (!file.name.toLowerCase().endsWith('.typ') || file.size > 2_000_000) throw new Error('Selecciona un archivo .typ de hasta 2 MB.'); if (uploadAttempt.current?.file !== file) uploadAttempt.current = { file, key:crypto.randomUUID() }; const uploaded = await api.request<CollaborationFile>('/api/v1/colaboracion', { method:'POST', idempotencyKey:uploadAttempt.current.key, body:{ scope:'empresa', id_persona:null, title:file.name, notes:'Plantilla para licitaciones', filename:file.name, content_base64:await fileBase64(file,2) } }); uploadAttempt.current = null; setConfirmed(false); setDefinition(d => ({ ...d, template_source_id:uploaded.id })); templates.reload(); } catch (reason) { setError(message(reason)); } finally { setUploading(false); } }} /></label><p>Seleccionada: {definition.template_source_id ? `archivo #${definition.template_source_id}` : 'ninguna'}</p><Notice>{templates.error}</Notice>{templates.data && <><Table columns={['Archivo', 'Acción']} rows={templates.data.items.filter(file => file.filename.toLowerCase().endsWith('.typ') && file.mime === 'text/plain').map(file => [file.filename,<button type="button" onClick={() => { setConfirmed(false); setDefinition(d => ({ ...d, template_source_id: file.id })); }}>Seleccionar plantilla</button>])} /><DocumentPagination page={templatePage} hasMore={templates.data.has_more} change={setTemplatePage} busy={busy || uploading} /></>}</div>
      <p className="document-wide document-note">Las variables y sus valores se crean en la pestaña Variables de cada expediente. Las definiciones anteriores de este formulario se conservan.</p>
      <label className="document-wide document-check"><input type="checkbox" required checked={confirmed} onChange={e => setConfirmed(e.target.checked)} /> Apruebo este esquema para su uso en expedientes.</label>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={!confirmed || uploading}>{busy ? 'Publicando…' : 'Aprobar y publicar'}</button></div>
    </fieldset></form>
  </Dialog>;
}
