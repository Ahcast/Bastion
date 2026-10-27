import { useRef, useState } from 'react';
import { message } from '../api.js';
import { moduleCan } from '../types.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { DocumentSection, DocumentPagination } from '../components/DocumentWorkspace.js';
import { kindLabels, sectionLabels } from '../tenders.js';
import type { FormDefinition, FormSection, FormVariable, PublishedForm, TenderPage, VariableKind } from '../tenders.js';

export default function FormLibrary() {
  const { api, access, notify } = useCore();
  const [page, setPage] = useState(1), [editing, setEditing] = useState<PublishedForm | 'new' | null>(null);
  const allowed = moduleCan(access, 'colaboracion', 'empresa', 'ver');
  const result = useLoad(signal => allowed ? api.request<TenderPage<PublishedForm>>(`/api/v1/colaboracion/formularios?page=${page}`, { signal }) : Promise.resolve(null), [page, allowed]);
  if (!allowed) return <Notice>No tienes lectura sobre la biblioteca de empresa.</Notice>;
  return <div className="document-workspace"><Heading title="Biblioteca de formularios" description="Incorpora cada formulario cuando lo necesites. Las versiones publicadas conservan sus variables y las copias existentes permanecen independientes." action={access.is_system_administrator && <button className="primary" onClick={() => setEditing('new')}>Nuevo formulario</button>} />
    <DocumentSection title="Formularios publicados" metadata={result.data ? `${result.data.items.length} en esta página` : undefined} list>
    <Notice>{result.error}</Notice>
    {result.data ? <><Table columns={['Formulario', 'Bloque', 'Revisión', 'Variables', 'Acción']} rows={result.data.items.map(form => [form.definition.title, sectionLabels[form.definition.section], form.revision, form.definition.variables.length, access.is_system_administrator ? <button onClick={() => setEditing(form)}>Nueva revisión</button> : 'Publicado'])} /><DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} /></> : !result.error && <Loading />}
    <p className="document-note">Esta biblioteca define la captura de datos. Los PDF e imágenes se incorporan dentro del expediente; las plantillas para generar el PDF final siguen pendientes.</p>
    </DocumentSection>
    {editing && <SchemaEditor initial={editing === 'new' ? null : editing} close={() => setEditing(null)} saved={() => { setEditing(null); result.reload(); notify('Esquema aprobado y publicado.'); }} />}
  </div>;
}

function SchemaEditor({ initial, close, saved }: { initial: PublishedForm | null; close: () => void; saved: () => void }) {
  const { api } = useCore(), { navigate } = useNavigationGuard();
  const [definition, setDefinition] = useState<FormDefinition>(() => initial ? structuredClone(initial.definition) : { title: '', section: 'legal', variables: [] });
  const baseline = useRef(JSON.stringify(definition));
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [confirmed, setConfirmed] = useState(false);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  useUnsavedChanges(JSON.stringify(definition) !== baseline.current, busy);
  const update = (index: number, change: Partial<FormVariable>) => { setConfirmed(false); setDefinition(d => ({ ...d, variables: d.variables.map((v, i) => i === index ? { ...v, ...change } : v) })); };
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
      <p className="document-wide document-note">Declara sólo los datos que pide este formulario. Publicar una revisión no modifica copias anteriores.</p>
      {definition.variables.map((variable, index) => <fieldset className="document-variable" key={index}><legend>Variable {index + 1}</legend>
        <label>Identificador<input value={variable.key} required pattern="[a-z][a-z0-9_]{0,63}" maxLength={64} onChange={e => update(index, { key: e.target.value })} /><small>Minúsculas, números y guion bajo; empieza por una letra.</small></label>
        <label>Etiqueta visible<input value={variable.label} required maxLength={255} onChange={e => update(index, { label: e.target.value })} /></label>
        <label>Tipo<select value={variable.kind} onChange={e => update(index, { kind: e.target.value as VariableKind })}>{Object.entries(kindLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label>
        <label className="document-check"><input type="checkbox" checked={variable.required} onChange={e => update(index, { required: e.target.checked })} /> Dato obligatorio para completar el formulario</label>
        <button type="button" onClick={() => { setConfirmed(false); setDefinition(d => ({ ...d, variables: d.variables.filter((_, i) => i !== index) })); }}>Quitar variable</button>
      </fieldset>)}
      <button type="button" disabled={definition.variables.length >= 100} onClick={() => { setConfirmed(false); setDefinition(d => ({ ...d, variables: [...d.variables, { key: '', label: '', kind: 'text', required: false }] })); }}>Añadir variable</button>
      <label className="document-wide document-check"><input type="checkbox" required checked={confirmed} onChange={e => setConfirmed(e.target.checked)} /> Apruebo este esquema para su uso en expedientes.</label>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={!confirmed}>{busy ? 'Publicando…' : 'Aprobar y publicar'}</button></div>
    </fieldset></form>
  </Dialog>;
}
