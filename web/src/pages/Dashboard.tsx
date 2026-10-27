import { useRef, useState } from 'react';
import { Notice, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { cents, chartPercent, dashboardCurrency, dashboardDate, dashboardMoney } from '../dashboard.js';
import type { DashboardPeriod, DashboardPoint, ExpenseDashboard, InvoiceDashboard, ModuleDashboard, OrderDashboard, ProcessDashboard, QuoteDashboard } from '../dashboard.js';
import type { Page } from '../types.js';
export const areas = [
  { id: 'procesos', module: 'procesos', scope: 'consulta', path: 'procesos', page: 'procesos', label: 'Procesos activos', title: 'Procesos', hint: 'Recepciones vigentes en el período', description: 'Oportunidades vigentes, entidades y fechas de recepción de ofertas.', chart: 'Recepciones de ofertas', breakdown: 'Plazos de recepción', table: 'Próximas recepciones', columns: ['Expediente', 'Entidad', 'Resumen', 'Recepción de ofertas', 'De interés'] },
  { id: 'cotizaciones', module: 'cotizaciones', scope: 'documentos', path: 'cotizaciones', page: 'cotizaciones', label: 'Cotizaciones', title: 'Cotizaciones', hint: 'Cotizaciones en el período', description: 'Cotizaciones, clientes, importes y seguimiento de ofertas.', chart: 'Monto cotizado en el tiempo', breakdown: 'Cotizaciones por estado', table: 'Cotizaciones recientes', columns: ['Cotización', 'Cliente', 'Fecha', 'Estado', 'Total'] },
  { id: 'ordenes', module: 'ordenes_compra', scope: 'documentos', path: 'ordenes-compra', page: 'ordenes_compra', label: 'Órdenes de compra', title: 'Órdenes de compra', hint: 'Órdenes y entregas', description: 'Órdenes, entregas y seguimiento de compras.', chart: 'Órdenes en el tiempo', breakdown: 'Órdenes por estado', table: 'Órdenes recientes', columns: ['Orden', 'Cliente', 'Fecha', 'Estado', 'Total'] },
  { id: 'facturas', module: 'facturas', scope: 'documentos', path: 'facturas', page: 'facturas', label: 'Borradores de factura', title: 'Facturas', hint: 'Borradores y saldos', description: 'Borradores, importes registrados y saldos.', chart: 'Monto en borradores', breakdown: 'Borradores por estado', table: 'Borradores recientes', columns: ['N.º orden', 'Cliente', 'Fecha', 'Estado', 'Total', 'Saldo'] },
  { id: 'gastos', module: 'gastos', scope: 'registros', path: 'gastos', page: 'gastos', label: 'Gastos', hint: 'Registro y control', title: 'Gastos', description: 'Gastos registrados, proveedores e importes.', chart: 'Gastos registrados en el tiempo', breakdown: 'Gastos por proveedor', table: 'Gastos recientes', columns: ['Factura', 'Fecha', 'Proveedor', 'Moneda', 'Total'] },
] as const;
const paths: Record<string, string> = {
  procesos: 'M4 5h16v16H4zM8 3v4m8-4v4M4 11h16m-11 4h2m2 0h2m-6 3h2',
  cotizaciones: 'M6 3h8l4 4v14H6zM14 3v5h4M9 12h6m-6 4h6',
  ordenes: 'm3 7 9-4 9 4v10l-9 4-9-4zM3 7l9 4 9-4m-9 4v10M7.5 5l9 4',
  facturas: 'M6 3h12v18l-3-2-3 2-3-2-3 2zM9 7h6m-6 4h6m-6 4h3',
  gastos: 'M3 7h18v13H3zM3 7V4h14v3m-1 5h5v4h-5z',
  arrow: 'M5 12h14m-5-5 5 5-5 5', clock: 'M12 7v5l3 2M20 12a8 8 0 1 1-16 0 8 8 0 0 1 16 0', chart: 'M4 4v16h16m-11-4v-5m5 5V8m5 8V5'
};
function Icon({ name, className = '' }: { name: string; className?: string }) { return <svg className={`db-icon ${className}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="square" strokeLinejoin="miter" aria-hidden="true" focusable="false"><path d={paths[name]} /></svg>; }
function SectionTitle({ id, title, label }: { id: string; title: string; label: string }) { return <div className="db-section-heading"><h3 id={id}>{title}</h3><span className="db-meta">{label}</span></div>; }
function bucketLabel(bucket: string) {
  return bucket.length === 10 ? bucket.slice(8) : new Intl.DateTimeFormat('es-HN', { month: 'short', year: '2-digit', timeZone: 'UTC' }).format(new Date(`${bucket}-01T12:00:00Z`));
}
export function DashboardChart({ points, monetary, title }: { points: DashboardPoint[]; monetary: boolean; title: string }) {
  const daily = points.some(point => point.bucket.length === 10);
  const values = points.map(point => monetary ? cents(point.amount) ?? 0n : BigInt(Math.max(0, point.count)));
  const maximum = values.reduce((max, value) => value > max ? value : max, 0n);
  if (!maximum) return <div className="db-chart-message db-no-activity"><Icon name="chart" /><strong>Sin actividad en este período</strong><span>Los valores registrados son cero.</span></div>;
  return <div className="db-chart-scroll" tabIndex={0} role="region" aria-label={`${title}. Gráfico con desplazamiento horizontal.`}><ul className={`db-bars${daily ? ' is-daily' : ''}`} aria-label={title}>{points.map((point, index) => {
    const detail = `${point.bucket}: ${monetary ? dashboardMoney(point.amount) : point.count} · ${point.count} registros`;
    return <li key={point.bucket} title={detail}><span className="sr-only">{detail}</span><span className="db-bar-track" aria-hidden="true"><span style={{ height: `${chartPercent(values[index]!, maximum)}%` }} /></span><span className="db-bar-label" aria-hidden="true">{!daily || index % 5 === 0 || index === points.length - 1 ? bucketLabel(point.bucket) : '\u00a0'}</span></li>;
  })}</ul></div>;
}
export function DashboardContent({ area, data, openModule }: { area: typeof areas[number]; data: ModuleDashboard; openModule?: (page: Page) => void }) {
  const process = area.id === 'procesos' ? data as ProcessDashboard : null;
  const quotes = area.id === 'cotizaciones' ? data as QuoteDashboard : null;
  const orders = area.id === 'ordenes' ? data as OrderDashboard : null;
  const invoices = area.id === 'facturas' ? data as InvoiceDashboard : null;
  const expenses = area.id === 'gastos' ? data as ExpenseDashboard : null;
  const kpis: [string, string | number][] = process ? [['Procesos vigentes', process.metrics.active], ['De interés', process.metrics.marked], ['Entidades', process.metrics.entities], ['Recepciones hoy', process.metrics.today]]
    : quotes ? [['Cotizaciones', quotes.metrics.total], ['Monto cotizado', dashboardMoney(quotes.metrics.amount)]]
      : orders ? [['Órdenes', orders.metrics.total], ['Monto ordenado', dashboardMoney(orders.metrics.amount)]]
        : invoices ? [['Borradores', invoices.metrics.total], ['Monto en borradores', dashboardMoney(invoices.metrics.amount)]]
          : [['Gastos', expenses!.metrics.total], ...expenses!.metrics.currencies.map((row): [string, string | number] => [`Total ${row.currency}`, dashboardCurrency(row.amount, row.currency)])];
  const follow: readonly (readonly [string, number])[] = process ? ['Hoy', 'Mañana', 'Después'].map(label => [label, data.distribution.find(row => row.label === label)?.count ?? 0] as const)
    : expenses ? [['Registros', expenses.metrics.total], ['Monedas', expenses.metrics.currencies.length]]
      : data.distribution.map(row => [row.label, row.count] as const);
  const maximum = data.distribution.reduce((max, row) => Math.max(max, row.count), 0);
  const rows: string[][] = process ? process.recent.map(row => [row.expediente, row.entidad, row.resumen_corto, dashboardDate(row.fecha_recepcion, true), row.marked ? 'Sí' : 'No'])
    : quotes ? quotes.recent.map(row => [row.quote_number ?? 'Sin número', row.client_name, dashboardDate(row.quote_date), row.status, dashboardMoney(row.total)])
      : orders ? orders.recent.map(row => [row.n_orders, row.client_name, dashboardDate(row.order_date), row.status, dashboardMoney(row.total)])
        : invoices ? invoices.recent.map(row => [row.order_number || 'Sin número', row.client_name, dashboardDate(row.invoice_date), row.status, dashboardMoney(row.total), dashboardMoney(row.balance)])
          : expenses!.recent.map(row => [row.invoice_number || 'Sin factura', dashboardDate(row.expense_date), row.provider, row.currency, `${row.currency} ${row.total}`]);
  const monetary = !!quotes || !!orders || !!invoices;
  const withoutTotal = quotes ? quotes.metrics.without_total : invoices ? invoices.metrics.without_total : 0;
  return <><div className="db-live-heading"><span className="db-meta">Actualizado: {dashboardDate(data.as_of, true)} · Honduras</span>{openModule && <button type="button" onClick={() => openModule(area.page)}>Ver {area.title.toLowerCase()} <Icon name="arrow" /></button>}</div><div className="db-kpis">{kpis.map(([label, value]) => <div key={label}><span>{label}</span><strong>{value}</strong></div>)}</div>{withoutTotal > 0 && <p className="db-data-note">Hay registros sin total calculado; no se incluyen en los montos.</p>}{expenses && <p className="db-data-note">Los gastos se agrupan por moneda; los importes de monedas distintas no se suman.</p>}<div className="db-insights"><section className="db-section db-follow" aria-labelledby="db-attention-title"><SectionTitle id="db-attention-title" title="Seguimiento" label={area.title} /><dl>{follow.map(([label, count]) => <div key={label}><dt>{label}</dt><dd>{count}</dd></div>)}</dl></section><section className="db-section" aria-labelledby="db-chart-title"><SectionTitle id="db-chart-title" title={area.chart} label={data.window.period === 'month' ? 'Por día' : 'Por mes'} /><DashboardChart points={data.series} monetary={monetary} title={area.chart} /><p className="db-chart-caption">{process ? 'Fecha de recepción de ofertas; solo procesos vigentes.' : expenses ? 'Cantidad de gastos por fecha; los importes se separan por moneda.' : 'Fecha del documento; los importes se calculan en el servidor.'} {data.chart_truncated && 'Se muestran los primeros 120 meses con actividad; los totales incluyen todo el período.'}</p></section><section className="db-section" aria-labelledby="db-breakdown-title"><SectionTitle id="db-breakdown-title" title={area.breakdown} label="Distribución" />{maximum ? <ul className="db-distribution">{data.distribution.map(row => <li key={row.label}><div><span>{row.label}</span><strong>{row.count}</strong></div><div className="db-distribution-track" aria-hidden="true"><span style={{ width: `${chartPercent(BigInt(row.count), BigInt(maximum))}%` }} /></div></li>)}</ul> : <p className="db-data-note">Sin registros en este período.</p>}</section></div><section className="db-section db-records" aria-labelledby="db-records-title"><SectionTitle id="db-records-title" title={area.table} label="Hasta 8 registros" /><div className="db-table-wrap" tabIndex={0} role="region" aria-label={`${area.table}. Tabla con desplazamiento horizontal.`}><table className="db-table"><caption className="sr-only">{area.table}</caption><thead><tr>{area.columns.map(column => <th key={column} scope="col">{column}</th>)}</tr></thead><tbody>{rows.map((row, index) => { const record = data.recent[index]!; const key = 'proceso_id' in record ? record.proceso_id : String(record.id); return <tr key={key}>{row.map((cell, cellIndex) => <td key={cellIndex} className={cellIndex === row.length - 1 && monetary ? 'db-money-cell' : cellIndex === 2 && process ? 'db-summary-cell' : undefined}>{cell}</td>)}</tr>; })}{!rows.length && <tr><td colSpan={area.columns.length}><div className="db-records-empty"><Icon name={area.id} /><strong>Sin registros en este período</strong><p>Selecciona otro período o actualiza el resumen.</p></div></td></tr>}</tbody></table></div></section></>;
}
export default function Dashboard({ openModule }: { openModule?: (page: Page) => void }) {
  const { api, access } = useCore();
  const allowed = areas.map(entry => moduleCan(access, entry.module, entry.scope, 'ver'));
  const [selected, setSelected] = useState(() => Math.max(0, allowed.findIndex(Boolean))), [period, setPeriod] = useState<DashboardPeriod>('month');
  const tabs = useRef<(HTMLButtonElement | null)[]>([]), area = areas[selected]!;
  const processes = useLoad(signal => allowed[0] ? api.request<ModuleDashboard>(`/api/v1/procesos/dashboard?period=${period}`, { signal }) : Promise.resolve(null), [period, allowed[0]]);
  const quotes = useLoad(signal => allowed[1] ? api.request<ModuleDashboard>(`/api/v1/cotizaciones/dashboard?period=${period}`, { signal }) : Promise.resolve(null), [period, allowed[1]]);
  const orders = useLoad(signal => allowed[2] ? api.request<ModuleDashboard>(`/api/v1/ordenes-compra/dashboard?period=${period}`, { signal }) : Promise.resolve(null), [period, allowed[2]]);
  const invoices = useLoad(signal => allowed[3] ? api.request<ModuleDashboard>(`/api/v1/facturas/dashboard?period=${period}`, { signal }) : Promise.resolve(null), [period, allowed[3]]);
  const expenses = useLoad(signal => allowed[4] ? api.request<ModuleDashboard>(`/api/v1/gastos/dashboard?period=${period}`, { signal }) : Promise.resolve(null), [period, allowed[4]]);
  const loads = [processes, quotes, orders, invoices, expenses];
  const current = loads[selected]!;
  const cardValue = (index: number) => {
    const data = loads[index]!.data;
    if (!data) return undefined;
    const metrics = data.metrics;
    return 'active' in metrics ? metrics.active : metrics.total;
  };
  const status = (index: number) => !allowed[index] ? access.modules.some(module => module.module_id === areas[index]!.module) ? 'Sin permiso' : 'No disponible' : loads[index]!.error ? 'Error al actualizar' : loads[index]!.data ? 'Datos reales' : 'Cargando…';
  const date = new Intl.DateTimeFormat('es-HN', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric', timeZone: 'America/Tegucigalpa' }).format(new Date());
  return <div className="dashboard"><header className="db-heading"><div><p className="db-overline">{date}</p><h1>Inicio</h1><p>Tu operación, en una vista. Selecciona un área para explorar su resumen.</p></div><span className="db-core-label">Bastion <span>/</span> Vista general</span></header><div className="db-toolbar"><label>Período<select value={period} onChange={event => setPeriod(event.target.value as DashboardPeriod)}><option value="month">Mes actual</option><option value="year">Año actual</option><option value="all">Todo</option></select></label><button type="button" onClick={() => loads.forEach(load => load.reload())}>Actualizar</button><p>Calendario de Honduras; cada módulo calcula sus indicadores con sus propios registros.</p></div><div className="db-selector-heading"><h2>Resumen por área</h2><span className="db-meta">{allowed.filter(Boolean).length} áreas disponibles</span></div><div className="db-cards" role="tablist" aria-label="Área del panel de inicio">{areas.map((entry, index) => {
    const value = allowed[index] ? cardValue(index) : undefined;
    return <button key={entry.id} ref={el => { tabs.current[index] = el; }} type="button" className={`db-card${selected === index ? ' is-selected' : ''}`} role="tab" id={`db-tab-${entry.id}`} aria-selected={selected === index} aria-controls="dashboard-content" tabIndex={selected === index ? 0 : -1} onClick={() => setSelected(index)} onKeyDown={event => {
      let next: number;
      if (event.key === 'ArrowRight') next = (index + 1) % areas.length;
      else if (event.key === 'ArrowLeft') next = (index + areas.length - 1) % areas.length;
      else if (event.key === 'Home') next = 0;
      else if (event.key === 'End') next = areas.length - 1;
      else return;
      event.preventDefault(); setSelected(next); tabs.current[next]?.focus(); tabs.current[next]?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
    }}><span className="db-card-top"><span className="db-card-number">0{index + 1}</span><Icon name={entry.id} /></span><span className="db-card-label">{entry.label}</span><span className="db-card-value" aria-label={value === undefined ? 'Sin datos' : undefined}>{value ?? '—'}</span><span className="db-card-hint">{entry.hint}</span><span className="db-card-bottom"><span className="db-card-status">{status(index)}</span><span className="db-card-action"><Icon name="arrow" /></span></span></button>;
  })}</div><p className="db-swipe-hint">Desliza para ver las cinco áreas <Icon name="arrow" /></p><section id="dashboard-content" className="db-detail" role="tabpanel" aria-labelledby={`db-tab-${area.id}`} tabIndex={0}><div className="db-detail-heading"><div><p className="db-overline">Resumen del área</p><h2>{area.title}</h2><p>{area.description}</p></div><span className="db-badge">{status(selected)}</span></div>{allowed[selected] ? current.error ? <><Notice>{current.error}</Notice><button type="button" onClick={current.reload}>Reintentar</button></> : current.data ? <DashboardContent area={area} data={current.data} openModule={openModule} /> : <p role="status">Cargando resumen…</p> : <div className="db-availability"><Icon name="clock" /><p>{access.modules.some(module => module.module_id === area.module) ? 'No tienes permiso para consultar los datos de esta área.' : 'Este módulo no está disponible en el ambiente actual.'}</p></div>}</section></div>;
}
