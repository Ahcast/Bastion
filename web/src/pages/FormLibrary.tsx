import { useRef, useState } from 'react';
import { message } from '../api.js';
import { moduleCan } from '../types.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { DocumentSection, DocumentPagination } from '../components/DocumentWorkspace.js';
import { sectionLabels } from '../tenders.js';
import type { FormDefinition, FormSection, PublishedForm, TenderPage } from '../tenders.js';
import type { CollaborationFile, FilePage } from '../collaboration.js';

export default function FormLibrary() {
  const { api, access, notify } = useCore();
  const [page, setPage] = useState(1), [editing, setEditing] = useState<PublishedForm | 'new' | null>(null);
  const allowed = moduleCan(access, 'colaboracion', 'empresa', 'ver');
  const result = useLoad(signal => allowed ? api.request<TenderPage<PublishedForm>>(`/api/v1/colaboracion/formularios?page=${page}`, { signal }) : Promise.resolve(null), [page, allowed]);
  if (!allowed) return <Notice>No tienes lectura sobre la biblioteca de empresa.</Notice>;
  return <div className="document-workspace"><Heading title="Biblioteca de formularios" description="Incorpora cada formulario cuando lo necesites. Las variables y sus valores se definen en cada expediente." action={access.is_system_administrator && <button className="primary" onClick={() => setEditing('new')}>Nuevo formulario</button>} />
    <DocumentSection title="Formularios publicados" metadata={result.data ? `${result.data.items.length} en esta página` : undefined} list>
    <Notice>{result.error}</Notice>
    {result.data ? <><Table columns={['Formulario', 'Bloque', 'Revisión', 'Acción']} rows={result.data.items.map(form => [form.definition.title, sectionLabels[form.definition.section], form.revision, access.is_system_administrator ? <button onClick={() => setEditing(form)}>Nueva revisión</button> : 'Publicado'])} /><DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} /></> : !result.error && <Loading />}
    <p className="document-note">Selecciona un archivo .typ de Colaboración para cada formulario. Las variables se crean en cada expediente. La generación del PDF final sigue pendiente.</p>
    </DocumentSection>
    {editing && <SchemaEditor initial={editing === 'new' ? null : editing} close={() => setEditing(null)} saved={() => { setEditing(null); result.reload(); notify('Esquema aprobado y publicado.'); }} />}
  </div>;
}

function SchemaEditor({ initial, close, saved }: { initial: PublishedForm | null; close: () => void; saved: () => void }) {
  const { api } = useCore(), { navigate } = useNavigationGuard();
  const [definition, setDefinition] = useState<FormDefinition>(() => initial ? structuredClone(initial.definition) : { title: '', section: 'legal', variables: [] });
  const baseline = useRef(JSON.stringify(definition));
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [confirmed, setConfirmed] = useState(false);
  const [templatePage, setTemplatePage] = useState(1);
  const templates = useLoad(signal => api.request<FilePage<CollaborationFile>>(`/api/v1/colaboracion?trash=false&page=${templatePage}&page_size=25`, { signal }), [templatePage]);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  useUnsavedChanges(JSON.stringify(definition) !== baseline.current, busy);
  return <Dialog title={initial ? `Nueva revisión de ${initial.definition.title}` : 'Nuevo formulario'} close={() => { if (!busy) void navigate(close); }}>
    <form className="stack" onSubmit={async event => {
      event.preventDefault(); if (busy || !confirmed) return;
      const body = { form_id: initial?.id ?? null, expected_revision: initial?.revision ?? null, definition };
      const serialized = JSON.stringify(body);
      if (attempt.current?.body !== serialized) attempt.current = { body: serialized, key: crypto.randomUUID() };
      setBusy(true); setError('');
      try { await api.request('/api/v1/colaboracion/formularios', { method: 'POST', body, idempotencyKey: attempt.current.key }); baseline.current = JSON.stringify(definition); saved(); }
      catch (reason) { setError(message(reason)); } finally { setBusy(false); }
    }}><Notice>{error}</Notice><fieldset disabled={busy} className="document-fields">
      <label>Nombre del formulario<input value={definition.title} required maxLength={255} onChange={e => { setConfirmed(false); setDefinition(d => ({ ...d, title: e.target.value })); }} /></label>
      <label>Bloque<select value={definition.section} onChange={e => { setConfirmed(false); setDefinition(d => ({ ...d, section: e.target.value as FormSection })); }}>{Object.entries(sectionLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label>
      <div className="document-wide"><h3>Plantilla Typst de Colaboración</h3><p>Seleccionada: {definition.template_source_id ? `archivo #${definition.template_source_id}` : 'ninguna'}</p><Notice>{templates.error}</Notice>{templates.data && <><Table columns={['Archivo', 'Acción']} rows={templates.data.items.filter(file => file.filename.toLowerCase().endsWith('.typ') && file.mime === 'text/plain').map(file => [file.filename,<button type="button" onClick={() => { setConfirmed(false); setDefinition(d => ({ ...d, template_source_id: file.id })); }}>Seleccionar plantilla</button>])} /><DocumentPagination page={templatePage} hasMore={templates.data.has_more} change={setTemplatePage} busy={busy} /></>}</div>
      <p className="document-wide document-note">Las variables y sus valores se crean en la pestaña Variables de cada expediente. Las definiciones anteriores de este formulario se conservan.</p>
      <label className="document-wide document-check"><input type="checkbox" required checked={confirmed} onChange={e => setConfirmed(e.target.checked)} /> Apruebo este esquema para su uso en expedientes.</label>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={!confirmed}>{busy ? 'Publicando…' : 'Aprobar y publicar'}</button></div>
    </fieldset></form>
  </Dialog>;
}
