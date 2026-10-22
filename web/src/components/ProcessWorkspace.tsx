import { Fragment } from 'react';
import type { ReactNode } from 'react';
import type { ProcessFilters, ProcessRecord, ProcessSort, ProcessStatus } from '../processes.js';
import { processDate, processDay } from '../processes.js';
import { processItems } from '../process-items.js';

export function ProcessFilterBar({ values, entities, change }: { values: ProcessFilters; entities: string[]; change: (values: ProcessFilters) => void }) {
  return <div className="process-filters" role="search" aria-label="Filtros de procesos">
    <label>Entidad<select value={values.entidad} onChange={event => change({ ...values, entidad: event.target.value })}><option value="">Todas las entidades</option>{entities.map(name => <option key={name} value={name}>{name}</option>)}</select></label>
    <label>Palabras clave<input type="search" placeholder="Especificaciones o expediente" maxLength={200} value={values.q} onChange={event => change({ ...values, q: event.target.value })} /></label>
    <label>Ordenar por<select value={values.sort} onChange={event => change({ ...values, sort: event.target.value as ProcessSort })}>
      <option value="fecha_desc">Recepción: más recientes</option><option value="fecha_asc">Recepción: más antiguas</option><option value="puntaje_desc">Puntuación: mayor primero</option><option value="puntaje_asc">Puntuación: menor primero</option>
    </select></label>
    <label className="process-check"><input type="checkbox" checked={values.marked} onChange={event => change({ ...values, marked: event.target.checked })} />Solo marcados</label>
    <label className="process-check" title="Según el nombre de la entidad: municipalidad, alcaldía, municipio o municipal."><input type="checkbox" checked={values.hide_municipalities} onChange={event => change({ ...values, hide_municipalities: event.target.checked })} />Ocultar municipalidades</label>
  </div>;
}

export function ProcessInsights({ status }: { status: ProcessStatus | null }) {
  const max = Math.max(1, ...(status?.weekly.map(week => week.count) ?? []));
  return <section className="process-insights" aria-label="Estadísticas de procesos">
    <div className="process-weekly"><div className="process-section-head"><h2>Procesos nuevos por semana</h2><span>Últimas 6 semanas</span></div>
      {status ? <ol className="process-chart" aria-label="Procesos por semana, desde el lunes indicado">{status.weekly.map((week, index) => <li key={week.week_start} className={index === status.weekly.length - 1 ? 'is-current' : ''}>
        <span className="process-bar-count">{week.count}</span><div className="process-bar-track"><span style={{ height: `${week.count / max * 100}%` }} /></div><span className="process-bar-label">{processDay(week.week_start)}</span>
      </li>)}</ol> : <p className="process-stat-loading" role="status">Cargando estadísticas…</p>}
      <p className="process-caption">Primera detección · semanas de lunes a domingo</p>
    </div>
    <div className="process-deliveries"><div className="process-section-head"><h2>Próximas entregas</h2><span>Fecha de recepción</span></div><div className="process-due-grid">{['Hoy', 'Mañana', 'Pasado mañana'].map((label, index) => {
      const day = status?.due[index];
      return <div className="process-due" key={label}><span>{label}</span><strong>{day?.count ?? '—'}</strong><small>{day ? processDay(day.date) : '—'}</small></div>;
    })}</div><p className="process-caption">Resultados filtrados · hora de Honduras</p></div>
  </section>;
}

export function ProcessTable({ rows, selected, select, mark, canMark, busy, detail }: {
  rows: ProcessRecord[]; selected: string | null; select: (id: string | null) => void;
  mark: (row: ProcessRecord, marked: boolean) => Promise<void>; canMark: boolean; busy: boolean; detail: (row: ProcessRecord) => ReactNode;
}) {
  if (!rows.length) return <div className="empty">No hay procesos que coincidan con los filtros.</div>;
  return <div className="process-table-wrap"><table className="process-table"><caption className="process-sr-only">Procesos. Abre una fila para desplegar su análisis y descripción.</caption>
    <thead><tr>{['Proceso / entidad', 'Resumen', 'Calificación', 'Rubro', 'Fecha recepción', 'De interés', 'Detalle'].map(label => <th scope="col" key={label}>{label}</th>)}</tr></thead>
    <tbody>{rows.map(row => {
      const open = selected === row.proceso_id, detailId = `process-detail-${row.proceso_id}`, toggle = () => select(open ? null : row.proceso_id);
      return <Fragment key={row.proceso_id}><tr className={`process-record${open ? ' is-expanded' : ''}`} onClick={event => { if (!(event.target as Element).closest('input,button,a,label')) toggle(); }}>
        <td data-label="Proceso"><button className="process-code" aria-expanded={open} aria-controls={detailId} onClick={toggle}>{row.expediente}</button><span className="process-entity">{row.entidad}</span>{row.note && <span className="process-note-hint">Con nota compartida</span>}</td>
        <td data-label="Resumen"><span className="process-preview">{row.resumen_corto}</span></td>
        <td data-label="Calificación"><Score value={row.puntaje} /></td><td data-label="Rubro">{row.rubro}</td><td data-label="Fecha recepción"><time dateTime={row.fecha_recepcion}>{processDate(row.fecha_recepcion)}</time></td>
        <td data-label="De interés"><input className="process-mark" type="checkbox" aria-label={`De interés: ${row.expediente}`} checked={row.marked} disabled={busy || !canMark} onChange={event => { void mark(row, event.target.checked); }} /></td>
        <td className="process-toggle-cell"><button className="process-toggle" aria-label={`${open ? 'Cerrar' : 'Desplegar'} ${row.expediente}`} aria-expanded={open} aria-controls={detailId} onClick={toggle}>{open ? '−' : '+'}</button></td>
      </tr>{open && <tr className="process-detail-row"><td colSpan={7}><section id={detailId} aria-label={`Detalle de ${row.expediente}`}>{detail(row)}</section></td></tr>}</Fragment>;
    })}</tbody>
  </table></div>;
}

export function Score({ value }: { value: number }) { return <span className="process-score"><strong>{value}<small>/100</small></strong><span className="process-score-track" aria-hidden="true"><span style={{ width: `${value}%` }} /></span></span>; }

export function ProcessDetailContent({ row, noteEditor }: { row: ProcessRecord; noteEditor?: ReactNode }) {
  const items = processItems(row.source.items ?? '');
  return <div className="process-detail">
    <div className="process-facts"><div><span>Calificación IA</span><Score value={row.puntaje} /></div><div><span>Rubro</span><strong>{row.rubro}</strong></div><div><span>Fecha recepción</span><strong>{processDate(row.fecha_recepcion)}</strong></div><div><span>Entidad</span><strong>{row.entidad}</strong></div></div>
    <div className="process-detail-columns"><div><section><h3>Resumen</h3><p className="process-text">{row.resumen_corto}</p></section><section><h3>Descripción / especificaciones</h3><p className="process-text">{row.source.descripcion || 'Sin descripción fuente.'}</p></section><a href={row.source.url} target="_blank" rel="noopener noreferrer">Abrir en HonduCompras ↗</a></div>
      <div><section><h3>Ítems analizados <span className="process-item-count">{items.length}</span></h3>{items.length ? <ol className="process-items">{items.map((item, index) => <li key={index}>{item}</li>)}</ol> : <p>Sin ítems analizados.</p>}</section><section><h3>Nota compartida</h3>{noteEditor ?? <p className="process-text">{row.note || 'Sin nota.'}</p>}</section></div></div>
    <details className="process-source"><summary>Datos de extracción y análisis</summary><dl>{Object.entries(row.source).map(([key, value]) => <div key={key}><dt>{key}</dt><dd className="process-text">{value}</dd></div>)}</dl></details>
  </div>;
}
