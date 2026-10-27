import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { moduleCan } from '../types.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { DocumentSection, DocumentPagination } from '../components/DocumentWorkspace.js';
import TenderFiles from '../components/TenderFiles.js';
import { fieldValue, missingLabels, sectionLabels } from '../tenders.js';
import type { Expedient, ExpedientSummary, FormCopy, FormSection, FormValues, PublishedForm, TenderPage } from '../tenders.js';

export default function Licitaciones() {
  const { access } = useCore();
  const [id, setId] = useState<number | null>(null);
  if (!moduleCan(access, 'licitaciones', 'expedientes', 'ver')) return <Notice>No tienes acceso a Licitaciones.</Notice>;
  return <div className="document-workspace">{id === null ? <ExpedientList open={setId} /> : <ExpedientWorkspace key={id} id={id} back={() => setId(null)} />}</div>;
}
function ExpedientList({ open }: { open: (id: number) => void }) {
  const { api, access } = useCore();
  const [page, setPage] = useState(1), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const attempt = useRef<string | null>(null);
  const result = useLoad(signal => api.request<TenderPage<ExpedientSummary>>(`/api/v1/licitaciones?page=${page}`, { signal }), [page]);
  return <><Heading title="Licitaciones" description="Prepara y consulta tus expedientes con los formularios de la biblioteca." action={moduleCan(access, 'licitaciones', 'expedientes', 'editar') && <button className="primary" disabled={busy} onClick={async () => {
    if (busy) return; setBusy(true); setError(''); attempt.current ??= crypto.randomUUID();
    try { const record = await api.request<Expedient>('/api/v1/licitaciones', { method: 'POST', body: {}, idempotencyKey: attempt.current }); attempt.current = null; open(record.id); }
    catch (reason) { setError(message(reason)); } finally { setBusy(false); }
  }}>{busy ? 'Creando…' : 'Nuevo expediente'}</button>} />
    <DocumentSection title="Listado de expedientes" metadata={result.data ? `${result.data.items.length} en esta página` : undefined} list>
    <Notice>{error || result.error}</Notice>
    {result.data ? <><Table columns={['Expediente', 'Estado', 'Versión', 'Acción']} rows={result.data.items.map(e => [<div className="document-record"><strong>{e.name || "Sin nombre"}</strong><small className="document-id">#{e.id}</small></div>, <span className="document-state">{e.state === 'borrador' ? 'Borrador' : e.state}</span>, <span className="document-id">{e.version}</span>, <button onClick={() => open(e.id)}>Abrir expediente</button>])} /><DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} busy={busy} /></> : !result.error && <Loading />}
    </DocumentSection>
  </>;
}
type Tab = 'resumen' | 'variables' | FormSection | 'pdf';
function ExpedientWorkspace({ id, back }: { id: number; back: () => void }) {
  const { api, access, notify } = useCore(), { navigate } = useNavigationGuard();
  const result = useLoad(signal => api.request<Expedient>(`/api/v1/licitaciones/${id}`, { signal }), [id]);
  const [tab, setTab] = useState<Tab>('resumen'), [copyId, setCopyId] = useState<number | null>(null), [copying, setCopying] = useState(false), [deleting, setDeleting] = useState(false);
  const record = result.data;
  const editable = record?.state === 'borrador' && moduleCan(access, 'licitaciones', 'expedientes', 'editar');
  const tabs: { id: Tab; label: string }[] = [{ id: 'resumen', label: 'Resumen' }, { id: 'variables', label: 'Variables' }, ...Object.entries(sectionLabels).map(([key, label]) => ({ id: key as FormSection, label })), { id: 'pdf', label: 'PDF final' }];
  const selectedCopy = record?.forms.find(c => c.id === copyId);
  return <><Heading title={record?.name || `Expediente #${id}`} description={`Expediente #${id} · Cada formulario conserva su revisión de biblioteca.`} action={<button onClick={() => { void navigate(back); }}>Volver al listado</button>} />
    <Notice>{result.error}</Notice>{!record && !result.error && <Loading />}
    {record && <><div className="row section-tabs document-tabs" aria-label="Secciones del expediente">{tabs.map(entry => <button key={entry.id} aria-pressed={tab === entry.id} onClick={() => { void navigate(() => { setCopyId(null); setTab(entry.id); }); }}>{entry.label}</button>)}</div>
      <div className="document-details"><span className="document-metadata">{record.state === 'borrador' ? 'Borrador' : record.state} · Versión {record.version}</span><button onClick={() => { void navigate(result.reload); }}>Actualizar expediente</button></div>
      {tab === 'resumen' ? <DocumentSection title="Contenido del expediente"><ExpedientName key={record.version} record={record} editable={Boolean(editable)} saved={value => { result.setData(value); notify("Nombre del expediente guardado."); }} reload={() => { void navigate(result.reload); }} /><div className="stack"><p>{record.forms.length} formularios incorporados. {record.forms.reduce((n, c) => n + c.missing_required.length, 0)} valores obligatorios pendientes.</p><p>El expediente comienza vacío. Los campos se declaran al incorporar cada formulario en Colaboración → Biblioteca de formularios.</p><p>Lotes, ítems, generación PDF y adjudicación están pendientes de las siguientes etapas.</p>{moduleCan(access, 'licitaciones', 'expedientes', 'eliminar') && record.state === 'borrador' && <button className="danger" onClick={() => setDeleting(true)}>Eliminar borrador</button>}</div></DocumentSection>
        : tab === 'pdf' ? <DocumentSection title="PDF final"><p>La generación, el índice automático, el foliado y el historial se incorporarán al completar la infraestructura documental. Todavía no hay un PDF generado.</p></DocumentSection>
        : <DocumentSection title={tabs.find(t => t.id === tab)?.label ?? "Formularios"} list action={editable && tab !== 'variables' && moduleCan(access, 'colaboracion', 'empresa', 'ver') && <button onClick={() => { void navigate(() => { setCopyId(null); setCopying(true); }); }}>Incorporar formulario</button>}>
          {!moduleCan(access, 'colaboracion', 'empresa', 'ver') && tab !== 'variables' && <p className="muted">Para incorporar nuevos formularios necesitas acceso a Colaboración. Las copias existentes siguen disponibles.</p>}
          {(tab === 'legal' || tab === 'tecnica') && <><TenderFiles record={record} section={tab} changed={result.setData} start={action => { void navigate(() => { setCopyId(null); action(); }); }} /><h3 className="document-subheading">Formularios</h3></>}
          <Table columns={['Formulario', 'Revisión de origen', 'Obligatorios pendientes', 'Acción']} rows={record.forms.filter(c => tab === 'variables' || c.source.definition.section === tab).map(c => [c.source.definition.title, c.source.revision, c.missing_required.length, <button onClick={() => { void navigate(() => setCopyId(c.id)); }}>Abrir valores</button>])} />
          {selectedCopy && <ValuesEditor key={`${selectedCopy.id}:${record.version}`} copy={selectedCopy} record={record} editable={Boolean(editable)} saved={value => { result.setData(value); notify('Valores guardados.'); }} reload={() => { void navigate(result.reload); }} />}
        </DocumentSection>}
      {copying && (tab === 'legal' || tab === 'economica' || tab === 'tecnica') && <CopyPicker record={record} section={tab} close={() => setCopying(false)} saved={value => { result.setData(value); setCopying(false); notify('Copia independiente incorporada.'); }} />}
      {deleting && <DeleteExpedient record={record} close={() => setDeleting(false)} deleted={back} />}
    </>}
  </>;
}
function ExpedientName({ record, editable, saved, reload }: { record: Expedient; editable: boolean; saved: (value: Expedient) => void; reload: () => void }) {
  const { api } = useCore();
  const [name, setName] = useState(record.name ?? ''), [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const baseline = useRef(record.name ?? '');
  useUnsavedChanges(name !== baseline.current, busy);
  if (!editable) return <p>Nombre del expediente: {record.name || 'Sin nombre'}</p>;
  return <form className="stack document-name" onSubmit={async event => {
    event.preventDefault(); if (busy) return;
    if (!name.trim()) { setError('Escribe un nombre para el expediente.'); return; }
    setBusy(true); setError(''); setConflict(false);
    try {
      const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}`, { method: 'PATCH', body: { name }, ifMatch: record.version });
      baseline.current = updated.name; setName(updated.name); saved(updated);
    } catch (reason) {
      const stale = reason instanceof ApiError && reason.status === 412;
      setConflict(stale); setError(stale ? 'El expediente cambió. Conservamos tu nombre sin guardar; carga la versión actual antes de editar de nuevo.' : message(reason));
    } finally { setBusy(false); }
  }}><Notice>{error}</Notice>{conflict && <button type="button" disabled={busy} onClick={reload}>Cargar versión actual</button>}
    <fieldset className="document-fields" disabled={busy}>
      <label className="document-wide">Nombre del expediente<input name="expedient_name" value={name} required maxLength={255} placeholder="Asigna un nombre al expediente" onChange={event => setName(event.target.value)} /></label>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={name === baseline.current}>{busy ? 'Guardando…' : 'Guardar nombre'}</button></div>
    </fieldset>
  </form>;
}
function ValuesEditor({ copy, record, editable, saved, reload }: { copy: FormCopy; record: Expedient; editable: boolean; saved: (e: Expedient) => void; reload: () => void }) {
  const { api } = useCore();
  const [values, setValues] = useState<FormValues>(() => structuredClone(copy.values));
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const baseline = useRef(JSON.stringify(values));
  useUnsavedChanges(JSON.stringify(values) !== baseline.current, busy);
  return <form className="stack document-values" onSubmit={async event => {
    event.preventDefault(); if (busy || !editable) return; setBusy(true); setError(''); setConflict(false);
    try { const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/formularios/${copy.id}/valores`, { method: 'PUT', body: { values }, ifMatch: record.version }); baseline.current = JSON.stringify(values); saved(updated); }
    catch (reason) { setError(message(reason)); setConflict(reason instanceof ApiError && reason.status === 412); } finally { setBusy(false); }
  }}><h3>{copy.source.definition.title}</h3><Notice>{error}</Notice>{conflict && <button type="button" onClick={reload}>Cargar versión actual</button>}
    {copy.missing_required.length > 0 && <p className="document-note" role="status">Pendientes: {missingLabels(copy).join(', ')}. Puedes guardar avances.</p>}
    <fieldset disabled={busy || !editable} className="document-fields">{copy.source.definition.variables.map(v => <label className={v.kind === "text" ? "document-wide" : undefined} key={v.key}>{v.label}{v.required ? ' · obligatorio' : ''}
      {v.kind === 'boolean' ? <select value={values[v.key] === undefined ? '' : String(values[v.key])} onChange={e => setValues(current => ({ ...current, [v.key]: fieldValue(v.kind, e.target.value) }))}><option value="">Sin completar</option><option value="true">Sí</option><option value="false">No</option></select>
        : v.kind === 'text' ? <textarea value={String(values[v.key] ?? '')} maxLength={4000} onChange={e => setValues(current => ({ ...current, [v.key]: e.target.value }))} />
          : <input type={v.kind === 'date' ? 'date' : 'text'} inputMode={v.kind === 'decimal' ? 'decimal' : v.kind === 'integer' ? 'numeric' : undefined} maxLength={v.kind === 'decimal' ? 44 : 31} value={String(values[v.key] ?? '')} onChange={e => setValues(current => ({ ...current, [v.key]: e.target.value }))} />}
    </label>)}{!copy.source.definition.variables.length && <p>Este formulario no declara variables de captura.</p>}{editable && <div className="document-wide document-save"><button className="primary" type="submit">{busy ? 'Guardando…' : 'Guardar valores'}</button></div>}</fieldset>
  </form>;
}
function CopyPicker({ record, section, close, saved }: { record: Expedient; section: FormSection; close: () => void; saved: (e: Expedient) => void }) {
  const { api } = useCore();
  const [page, setPage] = useState(1), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const attempt = useRef<{ id: number; revision: number; key: string } | null>(null);
  const result = useLoad(signal => api.request<TenderPage<PublishedForm>>(`/api/v1/colaboracion/formularios?page=${page}`, { signal }), [page]);
  return <Dialog title="Incorporar formulario publicado" close={() => { if (!busy) close(); }}><Notice>{error || result.error}</Notice><p>Se guardará una copia independiente del esquema y su revisión.</p>
    {result.data ? <><Table columns={['Formulario', 'Bloque', 'Revisión', 'Acción']} rows={result.data.items.filter(f => f.definition.section === section).map(form => [form.definition.title, sectionLabels[form.definition.section], form.revision, <button disabled={busy} onClick={async () => {
      if (busy) return; setBusy(true); setError('');
      if (attempt.current?.id !== form.id || attempt.current.revision !== form.revision) attempt.current = { id: form.id, revision: form.revision, key: crypto.randomUUID() };
      try { saved(await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/formularios`, { method: 'POST', body: { source_id: form.id, revision: form.revision }, ifMatch: record.version, idempotencyKey: attempt.current.key })); }
      catch (reason) { setError(message(reason)); } finally { setBusy(false); }
    }}>Incorporar</button>])} /><DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} busy={busy} /></> : !result.error && <Loading />}
  </Dialog>;
}
function DeleteExpedient({ record, close, deleted }: { record: Expedient; close: () => void; deleted: () => void }) {
  const { api } = useCore();
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  return <Dialog title={`Eliminar borrador #${record.id}`} close={() => { if (!busy) close(); }}><Notice>{error}</Notice><p>Confirma que deseas eliminar este expediente en Borrador y sus copias de trabajo.</p><button className="danger" disabled={busy} onClick={async () => { if (busy) return; setBusy(true); setError(''); try { await api.request(`/api/v1/licitaciones/${record.id}`, { method: 'DELETE', ifMatch: record.version, body: { confirm_id: record.id } }); deleted(); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>{busy ? 'Eliminando…' : 'Confirmar eliminación'}</button></Dialog>;
}

export function TenderSettings() {
  return <section className="card stack"><h2>Licitaciones</h2><p>Los formularios se incorporan individualmente en la biblioteca de Colaboración. El expediente conserva sus valores y copias independientes.</p><p>Entradas: 20 MB por archivo. Presupuestos técnicos independientes: 500 MB para expediente y 500 MB para PDF final. Los archivos legales y técnicos se incorporan desde PC o Colaboración. La generación del PDF final sigue pendiente.</p><p>Los ajustes editables del módulo se definirán cuando sean necesarios.</p></section>;
}
