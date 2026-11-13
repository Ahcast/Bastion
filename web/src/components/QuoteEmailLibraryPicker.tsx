import { useState } from 'react';
import { Dialog, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { DocumentPagination } from './DocumentWorkspace.js';
import type { CollaborationFile, FilePage } from '../collaboration.js';
import { fileSize } from '../quoteAttachments.js';

export function QuoteEmailLibraryPicker({ initial, maxFiles, maxBytes, apply, close }: {
  initial: CollaborationFile[]; maxFiles: number; maxBytes: number;
  apply: (files: CollaborationFile[]) => void; close: () => void;
}) {
  const { api } = useCore();
  const [selected, setSelected] = useState(initial);
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [page, setPage] = useState(1);
  const loaded = useLoad(signal => api.request<FilePage>(`/api/v1/colaboracion?trash=false&search=${encodeURIComponent(query)}&page=${page}&page_size=25`, { signal }), [query, page]);
  const bytes = selected.reduce((sum, file) => sum + file.size_bytes, 0);
  const allowed = selected.length <= maxFiles && bytes <= maxBytes;
  const pdfs = loaded.data?.items.filter(file => file.mime === 'application/pdf' && !file.trashed_at) ?? [];
  return <Dialog title="PDF de Colaboración" close={close}>
    <p className="quote-note">Selecciona varios PDF de las carpetas que puedes leer. Se agregarán sólo a este correo.</p>
    <form className="row" onSubmit={event => { event.preventDefault(); setQuery(search); setPage(1); }}><label>Buscar PDF<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label><button type="submit">Buscar</button></form>
    <Notice>{loaded.error}</Notice>
    {loaded.data ? <><Table columns={['Seleccionar', 'PDF', 'Tamaño']} rows={pdfs.map(file => {
      const checked = selected.some(entry => entry.id === file.id);
      const disabled = !checked && (selected.length >= maxFiles || bytes + file.size_bytes > maxBytes || file.size_bytes === 0);
      return [<input type="checkbox" aria-label={`Seleccionar ${file.title || file.filename}`} checked={checked} disabled={disabled} onChange={event => { const check = event.target.checked; setSelected(current => check ? [...current, file] : current.filter(entry => entry.id !== file.id)); }} />, <span>{file.title || file.filename}{file.title && <small className="document-id">{file.filename}</small>}</span>, fileSize(file.size_bytes)];
    })} />{pdfs.length === 0 && <p className="empty">No hay PDF disponibles en esta página.</p>}<DocumentPagination page={page} hasMore={loaded.data.has_more} change={setPage} /></> : !loaded.error && <Loading />}
    <p className="quote-note">{selected.length} PDF seleccionados · {fileSize(bytes)}. Disponible: {Math.max(0, maxFiles)} archivos y {fileSize(Math.max(0, maxBytes))}.</p>
    <div className="actions"><button type="button" onClick={close}>Cancelar</button><button type="button" className="primary" disabled={!allowed} onClick={() => { apply(selected); close(); }}>Agregar seleccionados</button></div>
  </Dialog>;
}
