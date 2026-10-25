import { useEffect, useRef, useState } from 'react';
import { message } from '../api.js';
import { Dialog, Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import { collaborationCan, contentBlob, fileBase64 } from '../collaboration.js';
import type { CollaborationFile, FileContent, FilePage, FilePolicy } from '../collaboration.js';

export default function Colaboracion() {
  const { access } = useCore();
  if (!collaborationCan(access)) return <Notice>No tienes acceso a Colaboración.</Notice>;
  return <Workspace />;
}
function Workspace() {
  const { api, access, notify } = useCore();
  const [scope, setScope] = useState<'empresa' | 'persona'>(() => moduleCan(access, 'colaboracion', 'empresa', 'ver') ? 'empresa' : 'persona');
  const [trash, setTrash] = useState(false), [purging, setPurging] = useState(false);
  const policy = useLoad(signal => api.request<FilePolicy>('/api/v1/colaboracion/settings', { signal }));
  const [person, setPerson] = useState<{ id: number; full_name: string } | null>(null);
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1);
  const [title, setTitle] = useState(''), [notes, setNotes] = useState(''), [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [removing, setRemoving] = useState<CollaborationFile | null>(null);
  const [preview, setPreview] = useState<{ title: string; url: string; mime: string; text?: string } | null>(null);
  const fileInput = useRef<HTMLInputElement>(null), attempt = useRef<{ body: string; key: string } | null>(null);
  const folder = scope === 'empresa' ? 'empresa' : person ? `persona:${person.id}` : '';
  const editable = !trash && Boolean(folder) && moduleCan(access, 'colaboracion', folder, 'editar');
  useUnsavedChanges(Boolean(title || notes || file), busy);
  const result = useLoad(signal => api.request<FilePage>(`/api/v1/colaboracion?trash=${trash}&scope=${scope}${person && scope === 'persona' ? `&id_persona=${person.id}` : ''}&search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [trash, scope, person?.id, query, page]);
  useEffect(() => () => { if (preview) URL.revokeObjectURL(preview.url); }, [preview]);
  const read = async (record: CollaborationFile, show: boolean) => {
    setBusy(true); setError('');
    try {
      const content = await api.request<FileContent>(`/api/v1/colaboracion/${record.id}/${show ? 'preview' : 'download'}`);
      const blob = contentBlob(content, show), url = URL.createObjectURL(blob);
      if (show) setPreview({ title: record.title, url, mime: content.mime, text: content.mime === 'text/plain' ? await blob.text() : undefined });
      else { const link = document.createElement('a'); link.href = url; link.download = record.filename; document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 60_000); }
    } catch (reason) { setError(message(reason)); } finally { setBusy(false); }
  };
  return <><Heading title="Colaboración" description="Archivos comunes de empresa y documentos organizados por persona." action={<button disabled={busy} onClick={result.reload}>Actualizar</button>} />
    <div className="collaboration-folders" role="group" aria-label="Vista de archivos"><button aria-pressed={!trash} disabled={busy} onClick={() => { setTrash(false); setPage(1); }}>Archivos</button><button aria-pressed={trash} disabled={busy} onClick={() => { setTrash(true); setPage(1); }}>Papelera</button></div>
    <div className="collaboration-folders" role="group" aria-label="Carpetas"><button aria-pressed={scope === 'empresa'} disabled={busy || !moduleCan(access, 'colaboracion', 'empresa', 'ver')} onClick={() => { setScope('empresa'); setPage(1); }}>Empresa</button><button aria-pressed={scope === 'persona'} disabled={busy} onClick={() => { setScope('persona'); setPage(1); }}>Personas</button></div>
    {scope === 'persona' && <PersonFolders selected={person} choose={value => { setPerson(value); setPage(1); }} disabled={busy} />}
    <form className="row collaboration-search" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar archivo<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} placeholder="Título, nombre o notas" /></label><button disabled={busy} type="submit">Filtrar</button><button disabled={busy} type="button" onClick={() => { setSearch(''); setQuery(''); setPage(1); setPerson(null); }}>Limpiar</button></form>
    <Notice>{error || result.error}</Notice>
    {editable && <section className="card collaboration-upload"><h2>Subir archivo · {scope === 'empresa' ? 'Empresa' : person?.full_name}</h2><form onSubmit={async event => {
      event.preventDefault(); if (busy) return; setBusy(true); setError('');
      try {
        if (!file || !title.trim()) throw new Error('Selecciona un archivo e indica un título.');
        const input = { scope, id_persona: scope === 'persona' ? person?.id : null, title, notes, filename: file.name, content_base64: await fileBase64(file, policy.data?.max_file_mb ?? 50) };
        const body = JSON.stringify(input);
        if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
        await api.request('/api/v1/colaboracion', { method: 'POST', body: input, idempotencyKey: attempt.current.key });
        setTitle(''); setNotes(''); setFile(null); attempt.current = null; if (fileInput.current) fileInput.current.value = '';
        setPage(1); result.reload(); notify('Archivo guardado.');
      } catch (reason) { setError(message(reason)); } finally { setBusy(false); }
    }}><fieldset disabled={busy} className="form-fields"><label>Título<input name="title" required maxLength={255} value={title} onChange={event => setTitle(event.target.value)} /></label><label>Archivo<input ref={fileInput} name="file" type="file" accept={policy.data?.allowed_extensions?.map(ext => `.${ext}`).join(',')} required onChange={event => setFile(event.target.files?.[0] ?? null)} /><small>Hasta {policy.data?.max_file_mb ?? 50} MB por archivo.</small></label><label>Notas<textarea name="notes" maxLength={4000} rows={3} value={notes} onChange={event => setNotes(event.target.value)} /></label><button className="primary" type="submit">{busy ? 'Procesando…' : 'Guardar / subir'}</button></fieldset></form></section>}
    {!trash && scope === 'persona' && !person && collaborationCan(access, 'editar') && <p>Abre una carpeta de persona para subir sus documentos.</p>}
    {result.data ? <><Table columns={['Título', 'Carpeta', 'Archivo', 'Tamaño', 'Fecha', 'Notas', ...(trash ? ['Purga programada'] : []), 'Acciones']} rows={result.data.items.map(record => [record.title, record.scope === 'empresa' ? 'Empresa' : (person?.id === record.id_persona ? person.full_name : `Persona #${record.id_persona}`), record.filename, `${(record.size_bytes / 1024).toFixed(1)} KiB`, new Intl.DateTimeFormat('es-HN', { dateStyle: 'short', timeStyle: 'short', timeZone: 'America/Tegucigalpa' }).format(new Date(record.created_at)), record.notes || '—', ...(trash ? [record.purge_after ? new Intl.DateTimeFormat('es-HN', { dateStyle: 'short', timeStyle: 'short', timeZone: 'America/Tegucigalpa' }).format(new Date(record.purge_after)) : '—'] : []), <div className="actions">{!trash && <><button disabled={busy} onClick={() => void read(record, true)}>Ver</button><button disabled={busy} onClick={() => void read(record, false)}>Descargar</button></>}{moduleCan(access, 'colaboracion', record.scope === 'empresa' ? 'empresa' : `persona:${record.id_persona}`, 'eliminar') && <>{trash && <button disabled={busy} onClick={async () => { setBusy(true); setError(''); try { await api.request(`/api/v1/colaboracion/${record.id}/restore`, { method: 'POST' }); result.reload(); notify('Archivo restaurado.'); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>Restaurar</button>}<button className="danger" disabled={busy} onClick={() => { setPurging(trash); setRemoving(record); }}>{trash ? 'Eliminar definitivamente' : 'Eliminar'}</button></>}</div>])} /><nav className="row" aria-label="Paginación de archivos"><button disabled={page === 1 || busy} onClick={() => setPage(value => value - 1)}>Anterior</button><span>Página {page}</span><button disabled={!result.data.has_more || busy} onClick={() => setPage(value => value + 1)}>Siguiente</button></nav></> : !result.error && <Loading />}
    {removing && <Dialog title={purging ? 'Eliminar definitivamente' : 'Mover a papelera'} close={() => { if (!busy) setRemoving(null); }}><p>{purging ? `Se eliminará definitivamente «${removing.title}» y su archivo privado. Esta acción no tiene restauración.` : `«${removing.title}» se moverá a papelera durante ${policy.data?.trash_retention_days ?? 30} días. Puedes restaurarlo antes de su purga.`}</p><Notice>{error}</Notice><div className="actions"><button disabled={busy} onClick={() => setRemoving(null)}>Cancelar</button><button className="danger" disabled={busy} onClick={async () => { setBusy(true); setError(''); try { await api.request(`/api/v1/colaboracion/${removing.id}${purging ? '/purge' : ''}`, { method: purging ? 'POST' : 'DELETE', body: { confirm_id: removing.id } }); setRemoving(null); result.reload(); notify(purging ? 'Borrado definitivo registrado. Se retirará el archivo privado.' : 'Archivo movido a papelera.'); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>{purging ? 'Confirmar borrado definitivo' : 'Mover a papelera'}</button></div></Dialog>}
    {preview && <Dialog title={preview.title} close={() => setPreview(null)}>{preview.mime === 'text/plain' ? <pre className="collaboration-text">{preview.text}</pre> : preview.mime.startsWith('image/') ? <img className="collaboration-image" src={preview.url} alt={preview.title} /> : <iframe title={`Vista previa: ${preview.title}`} className="collaboration-pdf" src={preview.url} sandbox="" />}</Dialog>}
  </>;
}
function PersonFolders({ selected, choose, disabled }: { selected: { id: number; full_name: string } | null; choose: (person: { id: number; full_name: string } | null) => void; disabled: boolean }) {
  const { api } = useCore();
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1);
  const result = useLoad(signal => api.request<FilePage<{ id: number; full_name: string }>>(`/api/v1/colaboracion/personas?q=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);
  return <section className="card"><h2>Carpetas de personas</h2><form className="row" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar persona<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button disabled={disabled}>Buscar</button></form><Notice>{result.error}</Notice><div className="collaboration-folders"><button disabled={disabled} aria-pressed={!selected} onClick={() => choose(null)}>Todas las personas</button>{result.data?.items.map(person => <button disabled={disabled} aria-pressed={selected?.id === person.id} key={person.id} onClick={() => choose(person)}>{person.full_name}</button>)}</div>{selected && <p>Carpeta abierta: <strong>{selected.full_name}</strong></p>}{result.data && <nav className="row" aria-label="Paginación de personas"><button disabled={disabled || page === 1} onClick={() => setPage(value => value - 1)}>Personas anteriores</button><span>Página {page}</span><button disabled={disabled || !result.data.has_more} onClick={() => setPage(value => value + 1)}>Más personas</button></nav>}</section>;
}
