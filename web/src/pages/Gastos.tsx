import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { ApiError, message } from '../api.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { emptyExpense, expenseInput } from '../gastos.js';
import type { Expense, ExpenseInput, ExpensePage, ExpenseCalculation } from '../gastos.js';

const todayInHonduras = () => new Intl.DateTimeFormat('sv-SE', { timeZone: 'America/Tegucigalpa', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date());
const amountPattern = /^\d{1,12}(?:\.\d{1,4})?$/;

export default function Gastos() {
  const { access } = useCore();
  if (!moduleCan(access, 'gastos', 'registros', 'ver')) return <Notice>No tienes acceso a Gastos.</Notice>;
  return <Workspace />;
}

export function ExpenseSettings() {
  return <section className="card expense-settings"><h2>Gastos</h2><p>Registro general de gastos con filtros, importes e ISV calculados por el servidor.</p><p>No hay ajustes específicos confirmados. La asociación con órdenes/facturas y la lista de monedas permanecen pendientes de cotejo.</p></section>;
}

function Workspace() {
  const { api, access, notify } = useCore(), { navigate } = useNavigationGuard();
  const [search, setSearch] = useState(''), [query, setQuery] = useState(''), [dateFrom, setDateFrom] = useState(''), [dateTo, setDateTo] = useState(''), [queryFrom, setQueryFrom] = useState(''), [queryTo, setQueryTo] = useState(''), [page, setPage] = useState(1);
  const [saved, setSaved] = useState<Expense | null>(null), [data, setData] = useState<ExpenseInput>(() => emptyExpense(todayInHonduras())), [open, setOpen] = useState(false), [dirty, setDirty] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false);
  const attempt = useRef<{ body: string; key: string } | null>(null), heading = useRef<HTMLHeadingElement>(null);
  const editable = moduleCan(access, 'gastos', 'registros', 'editar'), removable = moduleCan(access, 'gastos', 'registros', 'eliminar');
  const [preview, setPreview] = useState<ExpenseCalculation | null>(null), [previewError, setPreviewError] = useState('');
  useUnsavedChanges(dirty, busy);
  useEffect(() => {
    setPreview(null); setPreviewError('');
    const body = expenseInput({ ...data, taxable_amount: data.taxable_amount.trim() || '0', exempt_amount: data.exempt_amount.trim() || '0' });
    if (!open || !editable || !body.expense_date || !body.invoice_number.trim() || !body.provider.trim() || !amountPattern.test(body.taxable_amount) || !amountPattern.test(body.exempt_amount) || (Number(body.taxable_amount) <= 0 && Number(body.exempt_amount) <= 0)) return;
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      void api.request<ExpenseCalculation>('/api/v1/gastos/calculo', { method: 'POST', body, signal: controller.signal }).then(value => { if (!controller.signal.aborted) setPreview(value); }).catch(reason => { if (!controller.signal.aborted) setPreviewError(message(reason)); });
    }, 400);
    return () => { window.clearTimeout(timer); controller.abort(); };
  }, [api, data, open, editable]);
  const result = useLoad(signal => {
    const params = new URLSearchParams({ search: query, page: String(page), page_size: '20' });
    if (queryFrom) params.set('date_from', queryFrom);
    if (queryTo) params.set('date_to', queryTo);
    return api.request<ExpensePage>(`/api/v1/gastos?${params}`, { signal });
  }, [query, queryFrom, queryTo, page]);

  const startNew = () => { setSaved(null); setData(emptyExpense(todayInHonduras())); setOpen(true); setDirty(false); setConflict(false); setError(''); attempt.current = null; heading.current?.focus(); };
  const openForEdit = async (id: number) => {
    setBusy(true); setError('');
    try { const expense = await api.request<Expense>(`/api/v1/gastos/${id}`); setSaved(expense); setData(expenseInput(expense)); setOpen(true); setDirty(false); setConflict(false); attempt.current = null; heading.current?.focus(); }
    catch (reason) { setError(message(reason)); }
    finally { setBusy(false); }
  };
  const closeEditor = () => { setOpen(false); setSaved(null); setDirty(false); setConflict(false); attempt.current = null; result.reload(); };
  const change = <K extends keyof ExpenseInput>(key: K, value: ExpenseInput[K]) => { setData(previous => ({ ...previous, [key]: value })); setDirty(true); };
  const applyFilters = (event: FormEvent) => { event.preventDefault(); setQuery(search); setQueryFrom(dateFrom); setQueryTo(dateTo); setPage(1); };
  const clearFilters = () => { setSearch(''); setQuery(''); setDateFrom(''); setDateTo(''); setQueryFrom(''); setQueryTo(''); setPage(1); };

  return <>
    <Heading title="Gastos" description="Registra facturas de gastos y consulta sus bases, ISV y total." action={editable && <button className="primary" disabled={busy} onClick={() => void navigate(startNew)}>Nuevo gasto</button>} />
    <Notice>{open ? result.error : error || result.error}</Notice>
    {open && <section className="expense-editor" aria-labelledby="expense-editor-title">
      <div className="expense-section-heading"><h2 id="expense-editor-title" ref={heading} tabIndex={-1}>{saved ? `Gasto · ${saved.invoice_number}` : 'Nuevo gasto'}</h2><span>REGISTRO</span></div>
      <p className="expense-note">Requiere fecha, número de factura, proveedor y un monto gravado o exento mayor que cero.</p>
      {conflict && <div className="expense-conflict" role="status"><p>Tus cambios se conservan. Abre la versión actual antes de guardar.</p><button type="button" disabled={busy} onClick={() => saved && void navigate(() => void openForEdit(saved.id))}>Abrir versión actual</button></div>}
      <form className="expense-form" onSubmit={async event => {
        event.preventDefault(); if (busy || conflict || !editable) return;
        const taxable = data.taxable_amount.trim() || '0', exempt = data.exempt_amount.trim() || '0';
        if (!data.expense_date || !data.invoice_number.trim() || !data.provider.trim()) { setError('Completa la fecha, el número de factura y el proveedor.'); return; }
        if (!amountPattern.test(taxable) || !amountPattern.test(exempt) || (Number(taxable) <= 0 && Number(exempt) <= 0)) { setError('Indica un importe gravado o exento mayor que cero con hasta cuatro decimales.'); return; }
        const body = expenseInput({ ...data, taxable_amount: taxable, exempt_amount: exempt });
        setBusy(true); setError('');
        try {
          let updated: Expense;
          if (saved) updated = await api.request<Expense>(`/api/v1/gastos/${saved.id}`, { method: 'PUT', body, ifMatch: saved.version });
          else {
            const encoded = JSON.stringify(body);
            if (!attempt.current || attempt.current.body !== encoded) attempt.current = { body: encoded, key: crypto.randomUUID() };
            updated = await api.request<Expense>('/api/v1/gastos', { method: 'POST', body, idempotencyKey: attempt.current.key });
          }
          setSaved(updated); setData(expenseInput(updated)); setDirty(false); setConflict(false); attempt.current = null; result.reload(); notify('Gasto guardado.');
        } catch (reason) { setError(message(reason)); if (reason instanceof ApiError && reason.status === 412) setConflict(true); }
        finally { setBusy(false); }
      }}>
        <fieldset className="expense-fields" disabled={busy || conflict || !editable}>
          <label>Fecha<input name="expense_date" type="date" min="1000-01-01" max="9999-12-31" required value={data.expense_date} onChange={event => change('expense_date', event.target.value)} /></label>
          <label>Número de factura<input name="invoice_number" maxLength={50} required value={data.invoice_number} onChange={event => change('invoice_number', event.target.value)} /></label>
          <label>Proveedor<input name="provider" maxLength={255} required value={data.provider} onChange={event => change('provider', event.target.value)} /></label>
          <label>Moneda<select name="currency" value={data.currency} onChange={() => change('currency', 'HNL')}><option value="HNL">HNL</option></select></label>
          <label>Base gravada (HNL)<input name="taxable_amount" inputMode="decimal" maxLength={17} value={data.taxable_amount} onChange={event => change('taxable_amount', event.target.value)} /></label>
          <label>Base exenta (HNL)<input name="exempt_amount" inputMode="decimal" maxLength={17} value={data.exempt_amount} onChange={event => change('exempt_amount', event.target.value)} /></label>
        </fieldset>
        {(preview || (saved && !dirty)) && <dl className="expense-totals" aria-label="Cálculo del gasto"><div><dt>ISV 15%</dt><dd>L {(preview || saved)!.isv_15}</dd></div><div><dt>Total</dt><dd>L {(preview || saved)!.total}</dd></div></dl>}
        <Notice>{previewError}</Notice>
        <Notice>{error}</Notice>
        <div className="expense-form-actions">{editable && <button type="submit" className="primary">{busy ? 'Guardando…' : saved ? 'Actualizar gasto' : 'Guardar gasto'}</button>}<button type="button" disabled={busy} onClick={() => void navigate(closeEditor)}>Cerrar</button></div>
      </form>
    </section>}
    <section className="expense-list" aria-labelledby="expense-list-title">
      <div className="expense-section-heading"><h2 id="expense-list-title">Listado de gastos</h2><span>{result.data ? `PÁGINA ${page}` : 'REGISTRO'}</span></div>
      <form className="expense-search" onSubmit={applyFilters}>
        <label>Buscar factura o proveedor<input value={search} maxLength={200} onChange={event => setSearch(event.target.value)} /></label>
        <label>Desde<input type="date" value={dateFrom} onChange={event => setDateFrom(event.target.value)} /></label>
        <label>Hasta<input type="date" value={dateTo} onChange={event => setDateTo(event.target.value)} /></label>
        <button type="submit" disabled={busy}>Buscar / filtrar</button><button type="button" disabled={busy} onClick={clearFilters}>Limpiar filtros</button>
      </form>
      {result.data ? <>
        <dl className="expense-summary" aria-label="Resumen de gastos filtrados"><div><dt>Registros</dt><dd>{result.data.summary.count}</dd></div><div><dt>Base gravada</dt><dd>L {result.data.summary.taxable_amount}</dd></div><div><dt>Base exenta</dt><dd>L {result.data.summary.exempt_amount}</dd></div><div><dt>ISV 15%</dt><dd>L {result.data.summary.isv_15}</dd></div><div><dt>Total</dt><dd>L {result.data.summary.total}</dd></div></dl>
        <Table columns={['Factura', 'Fecha', 'Proveedor', 'Moneda', 'Base gravada', 'Base exenta', 'ISV 15%', 'Total', 'Acciones']} rows={result.data.items.map(expense => [expense.invoice_number, expense.expense_date, expense.provider, expense.currency, expense.taxable_amount, expense.exempt_amount, expense.isv_15, expense.total, <div className="expense-actions">{editable && <button disabled={busy} onClick={() => void navigate(() => void openForEdit(expense.id))}>Editar</button>}{removable && <button className="danger" disabled={busy} onClick={async () => { if (!window.confirm(`¿Eliminar el gasto de factura ${expense.invoice_number}? Se conservará el registro para auditoría.`)) return; setBusy(true); setError(''); try { await api.request(`/api/v1/gastos/${expense.id}`, { method: 'DELETE' }); if (saved?.id === expense.id) closeEditor(); result.reload(); notify('Gasto eliminado.'); } catch (reason) { setError(message(reason)); } finally { setBusy(false); } }}>Eliminar</button>}{!editable && !removable && '—'}</div>])} />
        <nav className="expense-pagination" aria-label="Paginación de gastos"><button disabled={page === 1 || busy} onClick={() => setPage(value => value - 1)}>Anterior</button><span role="status">Página {page}</span><button disabled={!result.data.has_more || busy} onClick={() => setPage(value => value + 1)}>Siguiente</button></nav>
      </> : !result.error && <Loading />}
    </section>
  </>;
}
