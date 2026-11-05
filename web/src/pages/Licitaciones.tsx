import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { ApiError, message } from '../api.js';
import { moduleCan } from '../types.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { DocumentSection, DocumentPagination } from '../components/DocumentWorkspace.js';
import TenderFiles from '../components/TenderFiles.js';
import TenderLots from '../components/TenderLots.js';
import TenderPdfOrder from '../components/TenderPdfOrder.js';
import FormLibrary from './FormLibrary.js';
import { changesDdpPricing, fieldValue, kindLabels, sectionLabels } from '../tenders.js';
import type { Expedient, ExpedientSummary, GlobalVariable, FormSection, PublishedForm, TenderPage, VariableKind } from '../tenders.js';

export default function Licitaciones() {
  const { access } = useCore();
  const [id, setId] = useState<number | null>(null);
  if (!moduleCan(access, 'licitaciones', 'expedientes', 'ver')) return <Notice>No tienes acceso a Licitaciones.</Notice>;
  return <div className="document-workspace">{id === null ? <ExpedientList open={setId} /> : <ExpedientWorkspace key={id} id={id} back={() => setId(null)} />}</div>;
}
function stateLabel(state: string) {
  if (state === 'borrador') return 'Borrador';
  if (state === 'enviada') return 'Enviada';
  return state;
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
    {result.data ? <><Table columns={['Expediente', 'Estado', 'Versión', 'Acción']} rows={result.data.items.map(e => [<div className="document-record"><strong>{e.name || "Sin nombre"}</strong><small className="document-id">#{e.id}</small></div>, <span className="document-state">{stateLabel(e.state)}</span>, <span className="document-id">{e.version}</span>, <button onClick={() => open(e.id)}>Abrir expediente</button>])} /><DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} busy={busy} /></> : !result.error && <Loading />}
    </DocumentSection>
  </>;
}
type Tab = 'resumen' | 'variables' | FormSection | 'pdf';
function ExpedientWorkspace({ id, back }: { id: number; back: () => void }) {
  const { api, access, notify } = useCore(), { navigate, saveAll, saveError } = useNavigationGuard();
  const result = useLoad(signal => api.request<Expedient>(`/api/v1/licitaciones/${id}`, { signal }), [id]);
  const [tab, setTab] = useState<Tab>('resumen'), [copying, setCopying] = useState(false), [deleting, setDeleting] = useState(false);
  const [visited, setVisited] = useState<Set<Tab>>(() => new Set(['resumen']));
  const showTab = (next: Tab) => { setVisited(current => new Set([...current, next])); setTab(next); };
  const record = result.data;
  const currentRecord = useRef<Expedient | null>(null);
  if (record && (!currentRecord.current || record.version >= currentRecord.current.version)) currentRecord.current = record;
  const latest = () => currentRecord.current!;
  const saved = (value: Expedient) => { currentRecord.current = value; result.setData(value); };
  const [savingAll, setSavingAll] = useState(false);
  const [confirmSent, setConfirmSent] = useState(false);
  const [sendError, setSendError] = useState('');
  const markSent = async () => {
    setConfirmSent(false);
    setSavingAll(true);
    setSendError('');
    try {
      if (!await saveAll()) return;
      const updated = await api.request<Expedient>(`/api/v1/licitaciones/${id}/marcar-enviada`, {
        method: 'POST', ifMatch: latest().version, body: {},
      });
      saved(updated);
      notify('Licitación marcada como enviada.');
    } catch (reason) {
      setSendError(reason instanceof ApiError && reason.status === 409
        ? 'No se pudo marcar como enviada. Si hay un PDF en generación, espera a que termine. Si el estado cambió, vuelve a abrir el expediente.'
        : message(reason));
    } finally {
      setSavingAll(false);
    }
  };
  const [draftVariables, setDraftVariables] = useState<GlobalVariable[] | null>(null);
  const [draftLots, setDraftLots] = useState<{numero:number; items:number; tamano_tabla?:string}[]>([]);
  const editable = record?.state === 'borrador' && moduleCan(access, 'licitaciones', 'expedientes', 'editar');
  const tabs: { id: Tab; label: string }[] = [{ id: 'resumen', label: 'Resumen' }, { id: 'variables', label: 'Variables' }, ...Object.entries(sectionLabels).map(([key, label]) => ({ id: key as FormSection, label })), { id: 'pdf', label: 'PDF final' }];
  return <><Heading title={record?.name || `Expediente #${id}`} description={`Expediente #${id} · Cada formulario conserva su revisión de biblioteca.`} action={<div className="actions"><button disabled={savingAll} onClick={() => { void navigate(back); }}>Volver al listado</button>{editable && <button className="primary" disabled={savingAll} onClick={async () => { setSavingAll(true); try { if (await saveAll()) notify('Expediente guardado.'); } finally { setSavingAll(false); } }}>{savingAll ? 'Guardando…' : 'Guardar expediente'}</button>}{editable && <button disabled={savingAll} onClick={() => setConfirmSent(true)}>Marcar como enviada</button>}</div>} />
    <Notice>{result.error || saveError || sendError}</Notice>{!record && !result.error && <Loading />}
    {record && <><div className="row section-tabs document-tabs" aria-label="Secciones del expediente">{tabs.map(entry => <button key={entry.id} aria-pressed={tab === entry.id} onClick={() => showTab(entry.id)}>{entry.label}</button>)}</div>
      <div className="document-details"><span className="document-metadata">{stateLabel(record.state)} · Versión {record.version}</span></div>
      <div hidden={tab !== 'resumen'}><DocumentSection title="Contenido del expediente"><ExpedientName record={record} latest={latest} editable={Boolean(editable)} saved={saved} reload={() => { void navigate(result.reload); }} /><div className="stack"><p>{record.forms.length + (record.initial_forms?.length ?? 0)} formularios incorporados. {record.forms.reduce((n, c) => n + c.missing_required.length, 0)} valores obligatorios pendientes.</p><p>Las variables se definen una sola vez en la pestaña Variables y se comparten en el expediente.</p><p>Prepara los lotes y sus ítems en Oferta económica y genera el documento desde PDF final.</p>{moduleCan(access, 'licitaciones', 'expedientes', 'eliminar') && record.state === 'borrador' && <button className="danger" onClick={() => setDeleting(true)}>Eliminar borrador</button>}</div></DocumentSection></div>
      {visited.has('variables') && <div hidden={tab !== 'variables'}><DocumentSection title="Variables del expediente"><GlobalVariablesEditor record={record} latest={latest} onDraft={setDraftVariables} draftLots={draftLots} editable={Boolean(editable)} saved={saved} reload={() => { void navigate(result.reload); }} /></DocumentSection></div>}
      {visited.has('economica') && <div hidden={tab !== 'economica'}><DocumentSection title="Oferta económica"><TenderLots record={{ ...record, variables: draftVariables ?? record.variables }} latest={latest} onDraftInfo={setDraftLots} editable={Boolean(editable)} saved={saved} /></DocumentSection></div>}
      {(['legal', 'tecnica'] as const).filter(section => visited.has(section)).map(section => {
        const renderFiles = (initialRows: ReactNode[][] = [], choices: {key:string; label:string}[] = [], addInitial?: (key:string) => void) => <><TenderFiles record={record} section={section} changed={saved} start={action => action()} extraAction={editable && (choices.length > 0 || moduleCan(access, 'colaboracion', 'empresa', 'ver')) && <button onClick={() => setCopying(true)}>Agregar archivo Typst</button>} extraRows={[...initialRows, ...record.forms.filter(c => c.source.definition.section === section).map(c => [c.source.definition.title, '—', 'Typst · Biblioteca', moduleCan(access, 'licitaciones', 'expedientes', 'eliminar') && editable ? <button className="danger" onClick={async () => { if (!window.confirm(`¿Quitar «${c.source.definition.title}» del expediente?`)) return; try { saved(await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/formularios/${c.id}`, { method:'DELETE', ifMatch:latest().version, body:{confirm_id:c.id} })); } catch (reason) { notify(message(reason)); } }}>Quitar</button> : '—'])]} />{copying && tab === section && <CopyPicker record={record} section={section} choices={choices} addInitial={addInitial} close={() => setCopying(false)} saved={value => { saved(value); setCopying(false); notify('Archivo Typst incorporado.'); }} />}</>;
        return <div hidden={tab !== section} key={section}><DocumentSection title={sectionLabels[section]} list>{section === 'legal' ? <InitialForms record={record} latest={latest} editable={Boolean(editable)} saved={saved}>{renderFiles}</InitialForms> : renderFiles()}</DocumentSection></div>;
      })}
      {visited.has('pdf') && <div hidden={tab !== 'pdf'}><DocumentSection title="PDF final"><TenderPdfOrder record={record} latest={latest} editable={Boolean(editable)} saved={saved} /></DocumentSection></div>}
      {confirmSent && <Dialog title="Marcar como enviada" close={() => setConfirmSent(false)}>
        <p>Confirma que ya presentaste la oferta. Se guardarán los cambios pendientes y el expediente quedará en modo de consulta.</p>
        <div className="actions">
          <button onClick={() => setConfirmSent(false)}>Cancelar</button>
          <button className="primary" onClick={() => { void markSent(); }}>Confirmar estado Enviada</button>
        </div>
      </Dialog>}
      {deleting && <DeleteExpedient record={record} close={() => setDeleting(false)} deleted={back} />}
    </>}
  </>;
}
function InitialForms({ record, latest, editable, saved, children }: { record: Expedient; latest: () => Expedient; editable: boolean; saved: (value: Expedient) => void; children: (rows: ReactNode[][], choices: {key:string; label:string}[], add: (key:string) => void) => ReactNode }) {
  const { api } = useCore();
  const options = [{ key:'integridad', label:'Declaración de integridad' }, { key:'oferente', label:'Información sobre el oferente' }];
  const available = useLoad(signal => api.request<{form_key:string; enabled:boolean}[]>('/api/v1/licitaciones/plantillas-instaladas', { signal }));
  const [selected, setSelected] = useState<string[]>(record.initial_forms ?? []), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const baseline = useRef(JSON.stringify(selected));
  const dirty = JSON.stringify(selected) !== baseline.current;
  const save = async () => { setBusy(true); setError(''); try { const value = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/pdf/formularios-iniciales`, { method:'PUT', ifMatch:latest().version, body:{forms:selected} }); baseline.current = JSON.stringify(selected); saved(value); return true; } catch (reason) { setError(message(reason)); return false; } finally { setBusy(false); } };
  useUnsavedChanges(dirty, busy, save, 30);
  const eligible = options.filter(option => !selected.includes(option.key) && ((Array.isArray(available.data) && available.data.some(item => item.form_key === option.key && item.enabled)) || (record.initial_forms ?? []).includes(option.key)));
  return <><Notice>{error || available.error}</Notice>{children(selected.map(key => [options.find(item => item.key === key)?.label ?? key, '—', 'Typst · Instalado', editable ? <button type="button" disabled={busy} onClick={() => setSelected(current => current.filter(item => item !== key))}>Quitar</button> : '—']), eligible, key => setSelected(current => current.includes(key) ? current : [...current, key]))}</>;
}
function ExpedientName({ record, latest, editable, saved, reload }: { record: Expedient; latest: () => Expedient; editable: boolean; saved: (value: Expedient) => void; reload: () => void }) {
  const { api } = useCore();
  const [name, setName] = useState(record.name ?? ''), [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const baseline = useRef(record.name ?? '');
  const save = async () => {
    if (!name.trim()) { setError('Escribe un nombre para el expediente.'); return false; }
    setBusy(true); setError(''); setConflict(false);
    try {
      const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}`, { method: 'PATCH', body: { name }, ifMatch: latest().version });
      baseline.current = updated.name; setName(updated.name); saved(updated);
      return true;
    } catch (reason) {
      const stale = reason instanceof ApiError && reason.status === 412;
      setConflict(stale); setError(stale ? 'El expediente cambió. Conservamos tu nombre sin guardar; carga la versión actual antes de editar de nuevo.' : message(reason));
      return false;
    } finally { setBusy(false); }
  };
  useUnsavedChanges(name !== baseline.current, busy, save, 10);
  if (!editable) return <p>Nombre del expediente: {record.name || 'Sin nombre'}</p>;
  return <form className="stack document-name" onSubmit={event => { event.preventDefault(); if (!busy) void save(); }}><Notice>{error}</Notice>{conflict && <button type="button" disabled={busy} onClick={reload}>Cargar versión actual</button>}
    <fieldset className="document-fields" disabled={busy}>
      <label className="document-wide">Nombre del expediente<input name="expedient_name" value={name} required maxLength={255} placeholder="Asigna un nombre al expediente" onChange={event => setName(event.target.value)} /></label>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={name === baseline.current}>{busy ? 'Guardando…' : 'Guardar nombre'}</button></div>
    </fieldset>
  </form>;
}
function GlobalVariablesEditor({ record, latest, onDraft, draftLots, editable, saved, reload }: { record: Expedient; latest: () => Expedient; onDraft: (value: GlobalVariable[]) => void; draftLots: {numero:number;items:number;tamano_tabla?:string}[]; editable: boolean; saved: (value: Expedient) => void; reload: () => void }) {
  const { api } = useCore();
  const fixed: GlobalVariable[] = [
    {key:'lpn', label:'LPN', kind:'text', value:null}, {key:'inst', label:'Institución', kind:'text', value:null},
    {key:'dia', label:'Día', kind:'integer', value:null}, {key:'mes', label:'Mes', kind:'text', value:null},
    {key:'year', label:'Año', kind:'integer', value:null}, {key:'lotes', label:'Cantidad de lotes', kind:'integer', value:null},
    {key:'stylo', label:'Estilo', kind:'text', value:null}, {key:'destino', label:'Destino DDP (JICA)', kind:'text', value:null}, {key:'descripcion', label:'Descripción', kind:'text', value:null},
  ];
  const [variables, setVariables] = useState<GlobalVariable[]>(() => [...fixed.filter(f => (f.key !== 'destino' || record.variables?.some(v => v.key === 'stylo' && v.value === 'jica')) && !(record.variables ?? []).some(v => v.key === f.key)), ...structuredClone(record.variables ?? [])]);
  const [discard, setDiscard] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const baseline = useRef(JSON.stringify(variables));
  useEffect(() => { onDraft(variables); }, [variables, onDraft]);
  const change = (index: number, value: Partial<GlobalVariable>) => setVariables(current => { const next = current.map((item,i) => i === index ? {...item,...value} : item); if (next.some(v => v.key === 'stylo' && v.value === 'jica') && !next.some(v => v.key === 'destino')) next.push({...fixed.find(v => v.key === 'destino')!}); return next; });
  const save = async (confirmDiscard: boolean) => {
    setBusy(true); setError(''); setConflict(false);
    try {
      if (variables.some(v => v.key === 'stylo' && v.value === 'jica') && !String(variables.find(v => v.key === 'destino')?.value ?? '').trim()) throw new Error('Completa el destino DDP de JICA.');
      if (!confirmDiscard) {
        const current = latest();
        const oldStyle = current.variables?.find(v => v.key === 'stylo')?.value ?? null;
        const nextStyle = variables.find(v => v.key === 'stylo')?.value ?? null;
        const nextCount = Number(variables.find(v => v.key === 'lotes')?.value ?? 0);
        if (oldStyle !== nextStyle || Number(current.variables?.find(v => v.key === 'lotes')?.value ?? 0) > nextCount) {
          const lots = await api.request<{numero:number;items:number;tamano_tabla?:string}[]>(`/api/v1/licitaciones/${record.id}/lotes`);
          const affected = [...lots, ...draftLots].filter(l => ((l.numero > nextCount || !nextStyle) && (l.items > 0 || Number(l.tamano_tabla ?? '7') !== 7)) || (l.items > 0 && ((oldStyle === 'bid' && nextStyle === 'std') || changesDdpPricing(oldStyle,nextStyle)))).filter((l, i, all) => all.findIndex(other => other.numero === l.numero) === i);
          if (affected.length) { setDiscard(affected.map(l => `Lote ${l.numero}: ${l.items} ítem(s)${l.numero <= nextCount && nextStyle ? changesDdpPricing(oldStyle,nextStyle) ? ', todos sus ítems y precios por el cambio de estilo DDP' : ', país de origen y transporte' : ', todos sus datos y tamaño de tabla'}`)); return false; }
        }
      }
      const updated = await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/variables`, { method: 'PUT', body: { variables, confirm_discard:confirmDiscard }, ifMatch: latest().version }); baseline.current = JSON.stringify(variables); setDiscard(null); saved(updated); return true;
    } catch (reason) { setError(message(reason)); setConflict(reason instanceof ApiError && reason.status === 412); return false; } finally { setBusy(false); }
  };
  useUnsavedChanges(JSON.stringify(variables) !== baseline.current, busy, () => save(false), 20);
  return <form className="stack document-values" onSubmit={event => { event.preventDefault(); if (editable && !busy) void save(false); }}><p>Estas variables son compartidas por todo el expediente. En la plantilla Typst usa <code>#variables.nombre</code>, sustituyendo <code>nombre</code> por el identificador. Los valores se incorporan al generar el PDF.</p>
    <Notice>{error}</Notice>{conflict && <button type="button" disabled={busy} onClick={reload}>Cargar versión actual</button>}
    <fieldset disabled={busy || !editable} className="document-fields">
      {variables.map((variable, index) => <fieldset className="document-variable" key={index}><legend>Variable {index + 1}</legend>
        <label>Identificador<input value={variable.key} readOnly={fixed.some(f => f.key === variable.key)} required pattern="[a-z][a-z0-9_]{0,63}" maxLength={64} onChange={e => change(index, { key: e.target.value })} /></label>
        <label>Etiqueta<input value={variable.label} readOnly={fixed.some(f => f.key === variable.key)} required maxLength={255} onChange={e => change(index, { label: e.target.value })} /></label>
        <label>Tipo<select disabled={fixed.some(f => f.key === variable.key && f.kind === variable.kind)} value={variable.kind} onChange={e => change(index, { kind: e.target.value as VariableKind, value: null })}>{Object.entries(kindLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label>
        <label>Valor{variable.key === 'total_paginas' ? <input value="Se calculará al generar el PDF" readOnly /> : variable.key === 'mes' || variable.key === 'stylo' ? <select value={String(variable.value ?? '')} onChange={e => change(index, { value:e.target.value || null })}><option value="">Seleccionar</option>{(variable.key === 'mes' ? ['enero','febrero','marzo','abril','mayo','junio','julio','agosto','septiembre','octubre','noviembre','diciembre'] : ['bid','std','jica']).map(option => <option key={option} value={option}>{option}</option>)}</select> : variable.kind === 'boolean' ? <select value={variable.value === null ? '' : String(variable.value)} onChange={e => change(index, { value: e.target.value === '' ? null : fieldValue(variable.kind, e.target.value) })}><option value="">Sin completar</option><option value="true">Sí</option><option value="false">No</option></select> : <input type={variable.kind === 'date' ? 'date' : 'text'} value={String(variable.value ?? '')} onChange={e => change(index, { value: e.target.value === '' ? null : e.target.value })} />}</label>
        {!fixed.some(f => f.key === variable.key) && <button type="button" onClick={() => setVariables(current => current.filter((_, i) => i !== index))}>Quitar variable</button>}
      </fieldset>)}
      <button type="button" disabled={variables.length >= 100} onClick={() => setVariables(current => [...current, { key: '', label: '', kind: 'text', value: null }])}>Añadir variable</button>
      <div className="document-wide document-save"><button className="primary" type="submit" disabled={JSON.stringify(variables) === baseline.current}>{busy ? 'Guardando…' : 'Guardar variables'}</button></div>
    </fieldset>
    <p>Total de páginas: calculado al generar el PDF.</p>{discard && <Dialog title="Confirmar descarte de datos" close={() => setDiscard(null)}><p>Se descartarán los siguientes datos:</p><ul>{discard.map(line => <li key={line}>{line}</li>)}</ul><button type="button" disabled={busy} onClick={() => { void save(true); }}>Confirmar descarte y guardar</button></Dialog>}
    {record.forms.some(form => Object.keys(form.legacy_values ?? {}).length > 0) && <div><h3>Valores anteriores por formulario</h3><p>Se conservan para consulta. Copia el valor que elijas a la variable global.</p>{record.forms.map(form => Object.entries(form.legacy_values ?? {}).map(([key, value]) => <p key={`${form.id}:${key}`}>{form.source.definition.title} · {key}: {String(value)}</p>))}</div>}
  </form>;
}
function CopyPicker({ record, section, close, saved, choices, addInitial }: { record: Expedient; section: FormSection; close: () => void; saved: (e: Expedient) => void; choices: {key:string; label:string}[]; addInitial?: (key:string) => void }) {
  const { api, access } = useCore();
  const [page, setPage] = useState(1), [busy, setBusy] = useState(false), [error, setError] = useState('');
  const attempt = useRef<{ id: number; revision: number; key: string } | null>(null);
  const allowed = moduleCan(access, 'colaboracion', 'empresa', 'ver');
  const result = useLoad(signal => allowed ? api.request<TenderPage<PublishedForm>>(`/api/v1/colaboracion/formularios?page=${page}`, { signal }) : Promise.resolve(null), [page, allowed]);
  return <Dialog title="Agregar archivo Typst" close={() => { if (!busy) close(); }}><Notice>{error || result.error}</Notice>
    {result.data || !allowed || result.error ? <><Table columns={['Archivo', 'Sección', 'Revisión', 'Acción']} rows={[...choices.map(option => [option.label, sectionLabels[section], 'Instalado', <button disabled={busy} onClick={() => { addInitial?.(option.key); close(); }}>Agregar</button>]), ...(result.data?.items ?? []).filter(f => f.definition.section === section).map(form => [form.definition.title, sectionLabels[form.definition.section], form.revision, <button disabled={busy} onClick={async () => {
      if (busy) return; setBusy(true); setError('');
      if (attempt.current?.id !== form.id || attempt.current.revision !== form.revision) attempt.current = { id: form.id, revision: form.revision, key: crypto.randomUUID() };
      try { saved(await api.request<Expedient>(`/api/v1/licitaciones/${record.id}/formularios`, { method: 'POST', body: { source_id: form.id, revision: form.revision }, ifMatch: record.version, idempotencyKey: attempt.current.key })); }
      catch (reason) { setError(message(reason)); } finally { setBusy(false); }
    }}>Agregar</button>])]} />{result.data && <DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} busy={busy} />}</> : !result.error && <Loading />}
  </Dialog>;
}
function DeleteExpedient({ record, close, deleted }: { record: Expedient; close: () => void; deleted: () => void }) {
  const { api } = useCore();
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  return <Dialog title={`Eliminar borrador #${record.id}`} close={() => { if (!busy) close(); }}><Notice>{error}</Notice><p>Confirma que deseas eliminar este expediente en Borrador y sus copias de trabajo.</p><button className="danger" disabled={busy} onClick={async () => { if (busy) return; setBusy(true); setError(''); try { await api.request(`/api/v1/licitaciones/${record.id}`, { method: 'DELETE', ifMatch: record.version, body: { confirm_id: record.id } }); deleted(); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>{busy ? 'Eliminando…' : 'Confirmar eliminación'}</button></Dialog>;
}

export function TenderSettings() {
  const { api, access, notify } = useCore();
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  const installed = useLoad(signal => api.request<{form_key:string; enabled:boolean}[]>('/api/v1/licitaciones/plantillas-instaladas', { signal }));
  const labels: Record<string,string> = { integridad:'Declaración de integridad', oferente:'Información sobre el oferente' };
  const rows = (installed.data ?? []).map(form => [labels[form.form_key] ?? form.form_key, 'Documentación legal', form.enabled ? 'Disponible' : 'Retirado', access.is_system_administrator ? <button disabled={busy} onClick={async () => { setBusy(true); setError(''); try { await api.request(`/api/v1/licitaciones/plantillas-instaladas/${form.form_key}`, { method:'PUT', body:{enabled:!form.enabled} }); installed.reload(); notify(form.enabled ? 'Archivo retirado para expedientes nuevos.' : 'Archivo disponible.'); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>{form.enabled ? 'Quitar del catálogo' : 'Restaurar'}</button> : '—']);
  return <section className="card stack"><h2>Ajustes de Licitaciones</h2><p>Administra los archivos Typst disponibles para los expedientes.</p><Notice>{error || installed.error}</Notice><FormLibrary settings installedRows={rows} /></section>;
}
