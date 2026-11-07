import { useState } from 'react';
import { Heading, Loading, Notice, useCore } from '../ui.js';
import { moduleCan } from '../types.js';
import { processDate } from '../processes.js';
import { useShortlist } from './ShortlistAlerts.js';
import { interestTags, needsShortlistReview, shortlistMatches } from '../shortlist.js';
import type { InterestTag, ShortlistItem } from '../shortlist.js';

export default function LicitacionesProcesos() {
  const { access } = useCore();
  const { data, items, receipts, error, storageFailed, unread, reload, review } = useShortlist();
  const [query, setQuery] = useState(''), [tag, setTag] = useState<InterestTag | ''>('');
  const [onlyUnread, setOnlyUnread] = useState(false);
  const [limit, setLimit] = useState(25);
  if (!moduleCan(access, 'procesos', 'consulta', 'ver')) return <Notice>Necesitas acceso a la consulta de Procesos.</Notice>;
  const visible = items.filter(item => shortlistMatches(item, query, tag) && (!onlyUnread || needsShortlistReview(item, receipts)));
  const needsReview = visible.filter(item => needsShortlistReview(item, receipts));
  const stale = data?.generated_at && Date.parse(data.checked_at) - Date.parse(data.generated_at) > 2 * 60 * 60 * 1000;
  return <div className="shortlist-page">
    <Heading title="Licitación" description="Licitaciones vigentes, con prioridad para JICA, BID y UCP." action={<div className="actions">
      <button onClick={reload}>Actualizar</button>
      <button disabled={!needsReview.length} onClick={() => review(needsReview)}>Marcar resultados revisados</button>
    </div>} />
    <Notice>{error}</Notice>
    <Notice>{storageFailed ? 'La revisión se conserva durante esta sesión. El navegador no pudo guardarla para el próximo acceso.' : ''}</Notice>
    {data && data.availability !== 'available' && <Notice>{data.availability === 'missing'
      ? 'Todavía no hay una extracción de licitaciones publicada.'
      : 'La extracción publicada no está disponible. Actualiza la consulta para reintentar.'}</Notice>}
    {data?.scan_status === 'parcial' && <Notice>Extracción parcial. Se conservan las licitaciones vigentes ya detectadas mientras se completa el recorrido.</Notice>}
    {stale && <Notice>La última extracción tiene más de dos horas. Revisa su fecha antes de preparar una oferta.</Notice>}
    {!data && !error && <Loading />}
    {data?.availability === 'available' && <>
      <section className="shortlist-stats" aria-label="Resumen de licitaciones">
        <div><span>Por revisar</span><strong>{unread}</strong><small>Nuevas o con información actualizada</small></div>
        <div><span>Licitaciones vigentes</span><strong>{items.length}</strong><small>Todas las modalidades incluidas en Scout</small></div>
        <div><span>Última extracción</span><strong className="shortlist-last-run">{processDate(data.generated_at!)}</strong><small>Hora de Honduras</small></div>
      </section>
      <section className="shortlist-workspace" aria-label="Licitaciones vigentes">
        <div className="shortlist-filters">
          <label>Buscar<input type="search" value={query} onChange={event => { setQuery(event.target.value); setLimit(25); }} placeholder="Expediente, unidad, entidad o lugar" /></label>
          <label>Interés<select value={tag} onChange={event => { setTag(event.target.value as InterestTag | ''); setLimit(25); }}>
            <option value="">Todos</option><option>JICA</option><option>BID</option><option>UCP</option>
          </select></label>
          <label className="shortlist-checkbox"><input type="checkbox" checked={onlyUnread} onChange={event => { setOnlyUnread(event.target.checked); setLimit(25); }} />Solo por revisar</label>
          <p>{visible.length} resultados</p>
        </div>
        {!visible.length && <div className="empty">{items.length ? 'No hay licitaciones con estos filtros.' : 'La extracción publicada no contiene licitaciones vigentes.'}</div>}
        <div className="shortlist-list">{visible.slice(0, limit).map(item => <ShortlistCard key={item.id} item={item}
          reviewState={item.coincidencias_interes || item.posibles_interes ? receipts[item.id] === item.version ? 'Revisada' : receipts[item.id] ? 'Actualizada' : 'Nueva' : ''}
          review={() => review([item])} />)}</div>
        {visible.length > limit && <button className="shortlist-more" onClick={() => setLimit(value => value + 25)}>Mostrar más</button>}
      </section>
      <p className="shortlist-caption">Las marcas de revisión se guardan por usuario en este navegador. Las coincidencias indican menciones en la ficha. Japón se señala como posible JICA para verificar en los documentos.</p>
    </>}
  </div>;
}

function ShortlistCard({ item, reviewState, review }: { item: ShortlistItem; reviewState: string; review: () => void }) {
  const pending = Boolean(reviewState && reviewState !== 'Revisada');
  return <article className={`shortlist-card${pending ? ' is-unread' : ''}`}>
    <header><div><span className="shortlist-code">{item.expediente}</span><span className="shortlist-review">{reviewState}</span></div>
      <div className="shortlist-tags">{interestTags(item).map(tag => <span key={tag}>{item.posibles_interes.split(' | ').includes(tag) ? `Posible ${tag}` : tag}</span>)}</div></header>
    <h2>{item.objeto}</h2><p className="shortlist-entity">{item.entidad}</p>
    <dl className="shortlist-facts">
      <div><dt>Unidad de compra</dt><dd>{item.unidad_compra || 'Sin publicar'}</dd></div>
      <div><dt>Lugar de recepción</dt><dd>{item.lugar_recepcion || 'Sin publicar'}</dd></div>
      <div><dt>Recepción de ofertas</dt><dd>{processDate(item.fecha_recepcion)}</dd></div>
      <div><dt>Fuente de financiamiento</dt><dd>{item.fuente || 'Sin publicar'}{item.tipo_fuente && <small>{item.tipo_fuente}</small>}</dd></div>
    </dl>
    <details><summary>{item.evidencia_interes ? 'Ver coincidencias y datos del proceso' : 'Ver datos del proceso'}</summary>
      {item.evidencia_interes && <p className="shortlist-evidence">{item.evidencia_interes}</p>}
      <p>{item.modalidad} · {item.tipo_adquisicion} · {item.etapa}</p>
      <p>Ficha consultada el {processDate(item.extraido_en)}.</p>
    </details>
    <footer><a href={item.url} target="_blank" rel="noreferrer">Abrir en HonduCompras ↗</a>
      {pending && <button onClick={review}>Marcar revisada</button>}</footer>
  </article>;
}
