import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { moduleCan } from '../types.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { DocumentSection, DocumentPagination } from '../components/DocumentWorkspace.js';
import TenderFiles from '../components/TenderFiles.js';
import TenderLots from '../components/TenderLots.js';
import TenderPdfOrder from '../components/TenderPdfOrder.js';
import { fieldValue, kindLabels, sectionLabels } from '../tenders.js';
import type { Expedient, ExpedientSummary, GlobalVariable, FormSection, PublishedForm, TenderPage, VariableKind } from '../tenders.js';

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
  const [tab, setTab] = useState<Tab>('resumen'), [copying, setCopying] = useState(false), [deleting, setDeleting] = useState(false);
  const record = result.data;
  const editable = record?.state === 'borrador' && moduleCan(access, 'licitaciones', 'expedientes', 'editar');
  const tabs: { id: Tab; label: string }[] = [{ id: 'resumen', label: 'Resumen' }, { id: 'variables', label: 'Variables' }, ...Object.entries(sectionLabels).map(([key, label]) => ({ id: key as FormSection, label })), { id: 'pdf', label: 'PDF final' }];
  return <><Heading title={record?.name || `Expediente #${id}`} description={`Expediente #${id} · Cada formulario conserva su revisión de biblioteca.`} action={<button onClick={() => { void navigate(back); }}>Volver al listado</button>} />
    <Notice>{result.error}</Notice>{!record && !result.error && <Loading />}
    {record && <><div className="row section-tabs document-tabs" aria-label="Secciones del expediente">{tabs.map(entry => <button key={entry.id} aria-pressed={tab === entry.id} onClick={() => { void navigate(() => { setTab(entry.id); }); }}>{entry.label}</button>)}</div>
      <div className="document-details"><span className="document-metadata">{record.state === 'borrador' ? 'Borrador' : record.state} · Versión {record.version}</span><button onClick={() => { void navigate(result.reload); }}>Actualizar expediente</button></div>
      {tab === 'resumen' ? <DocumentSection title="Contenido del expediente"><ExpedientName key={record.version} record={record} editable={Boolean(editable)} saved={value => { result.setData(value); notify("Nombre del expediente guardado."); }} reload={() => { void navigate(result.reload); }} /><div className="stack"><p>{record.forms.length} formularios incorporados. {record.forms.reduce((n, c) => n + c.missing_required.length, 0)} valores obligatorios pendientes.</p><p>Las variables se definen una sola vez en la pestaña Variables y se comparten en el expediente.</p><p>Lotes, ítems, generación PDF y adjudicación están pendientes de las siguientes etapas.</p>{moduleCan(access, 'licitaciones', 'expedientes', 'eliminar') && record.state === 'borrador' && <button className="danger" onClick={() => setDeleting(true)}>Eliminar borrador</button>}</div></DocumentSection>
        : tab === 'pdf' ? <DocumentSection title="PDF final"><TenderPdfOrder record={record} editable={Boolean(editable)} saved={result.setData} /></DocumentSection>
        : tab === 'variables' ? <DocumentSection title="Variables del expediente"><GlobalVariablesEditor key={record.version} record={record} editable={Boolean(editable)} saved={value => { result.setData(value); notify('Variables del expediente guardadas.'); }} reload={() => { void navigate(result.reload); }} /></DocumentSection>
        : <DocumentSection title={tabs.find(t => t.id === tab)?.label ?? "Formularios"} list action={editable && moduleCan(access, 'colaboracion', 'empresa', 'ver') && <button onClick={() => { void navigate(() => { setCopying(true); }); }}>Incorporar formulario</button>}>
          {!moduleCan(access, 'colaboracion', 'empresa', 'ver') && <p className="muted">Para incorporar nuevos formularios necesitas acceso a Colaboración. Las copias existentes siguen disponibles.</p>}
          {(tab === 'legal' || tab === 'tecnica') && <><TenderFiles record={record} section={tab} changed={result.setData} start={action => { void navigate(() => { action(); }); }} /><h3 className="document-subheading">Formularios</h3></>}
          {tab === 'economica' && <TenderLots record={record} editable={Boolean(editable)} saved={result.setData} />}
          <Table columns={['Formulario', 'Revisión de origen', 'Obligatorios pendientes', 'Acción']} rows={record.forms.filter(c => c.source.definition.section === tab).map(c => [c.source.definition.title, c.source.revision, c.missing_required.length, <button onClick={() => { void navigate(() => setTab('variables')); }}>Ver variables</button>])} />
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
function GlobalVariablesEditor({ record, editable, saved, reload }: { record: Expedient; editable: boolean; saved: (value: Expedient) => void; reload: () => void }) {
  const { api } = useCore();
  const fixed: GlobalVariable[] = [
    {key:'lpn', label:'LPN', kind:'text', value:null}, {key:'inst', label:'Institución', kind:'text', value:null},
    {key:'dia', label:'Día', kind:'integer', value:null}, {key:'mes', label:'Mes', kind:'text', value:null},
    {key:'year', label:'Año', kind:'integer', value:null}, {key:'lotes', label:'Cantidad de lotes', kind:'integer', value:null},
    {key:'stylo', label:'Estilo', kind:'text', value:null}, {key:'descripcion', label:'Descripción', kind:'text', value:null},
  ];
  const [variables, setVariables] = useState<GlobalVariable[]>(() => [...fixed.filter(f => !(record.variables ?? []).some(v => v.key === f.key)), ...structuredClone(record.variables ?? [])]);
  const [discard, setDiscard] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const baseline = useRef(JSON.stringify(variables));
  useUnsavedChanges(JSON.stringify(variables) !== baseline.current, busy);
  const change = (index: number, value: Partial<GlobalVariable>) => setVariables(current => current.map((item, i) => i === index ? { ...item, ...value } : item));
  const save = async (confirmDiscard: boolean) => {
    setBusy(true); setError(''); setConflict(false);
    try {
      if (!confirmDiscard) {
        const oldStyle = record.variables?.find(v => v.key === 'stylo')?.value ?? null;
        const nextStyle = variables.find(v => v.key === 'stylo')?.value ?? null;
        const nextCount = Number(variables.find(v => v.key === 'lotes')?.value ?? 0);
        if (oldStyle !== nextStyle || Number(record.variables?.find(v => v.key === 'lotes')?.value ?? 0) > nextCount) {
          const lots = await api.request<{numero:number;items:number}[]>(`/api/v1/licitaciones/${record.id}/lotes`);
          const affected = lots.filter(l => l.items > 0 && (l.numero > nextCount || (oldStyle === 'bid' && nextStyle === 'std') || !nextStyle));
          if (affected.length) { setDiscard(affected.map(l => `Lote ${l.numero}: ${l.items} ítem(s)${l.numero <= nextCount && nextStyle ? ', país de origen y transporte' : ', todos sus datos'}`)); return; }
        }
      }
      const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/variables`, { method: 'PUT', body: { variables, confirm_discard:confirmDiscard }, ifMatch: record.version }); baseline.current = JSON.stringify(variables); setDiscard(null); saved(updated);
    } catch (reason) { setError(message(reason)); setConflict(reason instanceof ApiError && reason.status === 412); } finally { setBusy(false); }
  };
  return <form className="stack document-values" onSubmit={event => { event.preventDefault(); if (editable && !busy) void save(false); }}><p>Estas variables son compartidas por todo el expediente. En la plantilla Typst usa <code>#variables.nombre</code>, sustituyendo <code>nombre</code> por el identificador. La generación del PDF con estas variables está pendiente.</p>
    <Notice>{error}</Notice>{conflict && <button type="button" disabled={busy} onClick={reload}>Cargar versión actual</button>}
    <fieldset disabled={busy || !editable} className="document-fields">
      {variables.map((variable, index) => <fieldset className="document-variable" key={index}><legend>Variable {index + 1}</legend>
        <label>Identificador<input value={variable.key} readOnly={fixed.some(f => f.key === variable.key)} required pattern="[a-z][a-z0-9_]{0,63}" maxLength={64} onChange={e => change(index, { key: e.target.value })} /></label>
        <label>Etiqueta<input value={variable.label} readOnly={fixed.some(f => f.key === variable.key)} required maxLength={255} onChange={e => change(index, { label: e.target.value })} /></label>
        <label>Tipo<select disabled={fixed.some(f => f.key === variable.key)} value={variable.kind} onChange={e => change(index, { kind: e.target.value as VariableKind, value: null })}>{Object.entries(kindLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label>
        <label>Valor{variable.key === 'mes' || variable.key === 'stylo' ? <select value={String(variable.value ?? '')} onChange={e => change(index, { value:e.target.value || null })}><option value="">Seleccionar</option>{(variable.key === 'mes' ? ['enero','febrero','marzo','abril','mayo','junio','julio','agosto','septiembre','octubre','noviembre','diciembre'] : ['bid','std']).map(option => <option key={option} value={option}>{option}</option>)}</select> : variable.kind === 'boolean' ? <select value={variable.value === null ? '' : String(variable.value)} onChange={e => change(index, { value: e.target.value === '' ? null : fieldValue(variable.kind, e.target.value) })}><option value="">Sin completar</option><option value="true">Sí</option><option value="false">No</option></select> : <input type={variable.kind === 'date' ? 'date' : 'text'} value={String(variable.value ?? '')} onChange={e => change(index, { value: e.target.value === '' ? null : e.target.value })} />}</label>
        {!fixed.some(f => f.key === variable.key) && <button type="button" onClick={() => setVariables(current => current.filter((_, i) => i !== index))}>Quitar variable</button>}
      </fieldset>)}
      <button type="button" disabled={variables.length >= 100} onClick={() => setVariables(current => [...current, { key: '', label: '', kind: 'text', value: null }])}>Añadir variable</button>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={JSON.stringify(variables) === baseline.current}>{busy ? 'Guardando…' : 'Guardar variables'}</button></div>
    </fieldset>
    <p>Total de páginas: calculado al generar el PDF.</p>{discard && <Dialog title="Confirmar descarte de datos" close={() => setDiscard(null)}><p>Se descartarán los siguientes datos:</p><ul>{discard.map(line => <li key={line}>{line}</li>)}</ul><button type="button" disabled={busy} onClick={() => { void save(true); }}>Confirmar descarte y guardar</button></Dialog>}
    {record.forms.some(form => Object.keys(form.legacy_values ?? {}).length > 0) && <div><h3>Valores anteriores por formulario</h3><p>Se conservan para consulta. Copia el valor que elijas a la variable global.</p>{record.forms.map(form => Object.entries(form.legacy_values ?? {}).map(([key, value]) => <p key={`${form.id}:${key}`}>{form.source.definition.title} · {key}: {String(value)}</p>))}</div>}
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
  return <section className="card stack"><h2>Licitaciones</h2><p>Los formularios se incorporan individualmente en la biblioteca de Colaboración. El expediente conserva sus valores y copias independientes.</p><p>Entradas: 100 MB por archivo y 1 GB por expediente. El PDF final conserva su presupuesto independiente de 500 MB. Los archivos legales y técnicos se incorporan desde PC o Colaboración. La generación del PDF final sigue pendiente.</p><p>Los ajustes editables del módulo se definirán cuando sean necesarios.</p></section>;
}
