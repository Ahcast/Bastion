import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { DocumentPagination, DocumentSection } from './DocumentWorkspace.js';
import { Heading, Loading, Notice, Table, useCore, useLoad } from '../ui.js';
import { moduleCan } from '../types.js';
import { useUnsavedChanges } from '../navigationGuard.js';
import { InvoiceEditor } from '../pages/Facturas.js';
import type { InvoiceDraft, InvoiceInput } from '../invoices.js';
import { invoiceFromAward } from '../tenderAwards.js';
import type { Award, AwardSummary, AwardSelection, AvailableAwardLot, TenderInvoiceResult } from '../tenderAwards.js';
import type { Expedient, TenderPage } from '../tenders.js';

export default function TenderAwards({ record }: { record: Expedient }) {
  const { api, access, notify } = useCore();
  const [page, setPage] = useState(1);
  const [creating, setCreating] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState('');
  const [review, setReview] = useState<{ award: Award; seed: InvoiceInput } | null>(null);
  const [invoice, setInvoice] = useState<InvoiceDraft | null>(null);
  const result = useLoad(signal => api.request<TenderPage<AwardSummary>>(
    `/api/v1/licitaciones/${record.id}/adjudicaciones?page=${page}`, { signal }), [record.id, page]);
  const editable = moduleCan(access, 'licitaciones', 'expedientes', 'editar');
  const canInvoice = editable && moduleCan(access, 'facturas', 'documentos', 'ver')
    && moduleCan(access, 'facturas', 'documentos', 'editar')
    && moduleCan(access, 'clientes', 'catalogo', 'ver');

  const prepare = async (award: AwardSummary) => {
    setOpening(true);
    setError('');
    try {
      const existing = await api.request<TenderInvoiceResult | null>(
        `/api/v1/licitaciones/${record.id}/adjudicaciones/${award.id}/factura`);
      if (existing) {
        setInvoice(await api.request<InvoiceDraft>(`/api/v1/facturas/${existing.target_id}`));
      } else {
        const snapshot = await api.request<Award>(
          `/api/v1/licitaciones/${record.id}/adjudicaciones/${award.id}`);
        setReview({ award: snapshot, seed: invoiceFromAward(snapshot) });
      }
    } catch (reason) {
      setError(message(reason));
    } finally {
      setOpening(false);
    }
  };

  if (review || invoice) {
    const save = review ? async (input: InvoiceInput, key: string) => {
      const target = await api.request<TenderInvoiceResult>(
        `/api/v1/licitaciones/${record.id}/adjudicaciones/${review.award.id}/factura`, {
          method: 'POST', body: input, ifMatch: review.award.version, idempotencyKey: key,
        });
      return api.request<InvoiceDraft>(`/api/v1/facturas/${target.target_id}`);
    } : undefined;
    return <>
      {review && <p className="quote-note">Adjudicación #{review.award.id}. Elige un cliente del catálogo y revisa los precios e impuestos antes de guardar el borrador.</p>}
      <InvoiceEditor key={review ? `award-${review.award.id}` : `invoice-${invoice!.id}`}
        initial={invoice} seed={review?.seed} saveDraft={save}
        editable={canInvoice && (!invoice || invoice.status === 'Borrador' || invoice.status === 'Enviado')}
        close={() => { setReview(null); setInvoice(null); result.reload(); }} />
    </>;
  }

  return <>
    <Heading title="Adjudicaciones" description="Registra los ítems y cantidades ganadas y prepara su borrador de factura."
      action={editable && record.state === 'enviada' && <button className="primary" disabled={opening} onClick={() => setCreating(true)}>Registrar adjudicación</button>} />
    <Notice>{error || result.error}</Notice>
    {record.state !== 'enviada' && <p>Marca la oferta como Enviada antes de registrar sus adjudicaciones.</p>}
    <DocumentSection title="Adjudicaciones registradas" list>
      {result.data ? <>
        <Table columns={['Adjudicación', 'Ítems', 'Acción']} rows={result.data.items.map(award => [
          `#${award.id}`, award.item_count,
          <button disabled={!canInvoice || opening} onClick={() => void prepare(award)}>Preparar o abrir factura</button>,
        ])} />
        <DocumentPagination page={page} hasMore={result.data.has_more} change={setPage} busy={opening} />
      </> : !result.error && <Loading />}
    </DocumentSection>
    {creating && <AwardRegistration record={record} close={() => setCreating(false)} saved={() => {
      setCreating(false); result.reload(); notify('Adjudicación registrada.');
    }} />}
  </>;
}

function AwardRegistration({ record, close, saved }: {
  record: Expedient; close: () => void; saved: () => void;
}) {
  const { api } = useCore();
  const [number, setNumber] = useState(1);
  const [selection, setSelection] = useState<Record<string, AwardSelection>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const count = Number((record.variables ?? []).find(variable => variable.key === 'lotes')?.value ?? 0);
  const valid = Number.isSafeInteger(count) && count > 0 && count <= 1000;
  const lot = useLoad(signal => valid ? api.request<AvailableAwardLot>(
    `/api/v1/licitaciones/${record.id}/lotes/${number}/adjudicacion`, { signal }) : Promise.resolve(null),
  [record.id, number, valid]);
  const attempt = useRef<{ body: string; key: string } | null>(null);
  const selected = Object.values(selection);
  useUnsavedChanges(selected.length > 0, busy);

  const register = async () => {
    if (busy || selected.length === 0) return;
    setBusy(true);
    setError('');
    const input = { items: selected };
    const body = JSON.stringify(input);
    if (!attempt.current || attempt.current.body !== body) attempt.current = { body, key: crypto.randomUUID() };
    try {
      await api.request<Award>(`/api/v1/licitaciones/${record.id}/adjudicaciones`, {
        method: 'POST', body: input, ifMatch: record.version, idempotencyKey: attempt.current.key,
      });
      setSelection({});
      saved();
    } catch (reason) {
      setError(reason instanceof ApiError && reason.status === 409
        ? 'Las cantidades disponibles cambiaron o la oferta aún no está Enviada. Revisa los ítems y vuelve a intentar.'
        : message(reason));
      lot.reload();
    } finally {
      setBusy(false);
    }
  };

  return <DocumentSection title="Seleccionar ítems y cantidades">
    <p>Selecciona hasta 200 ítems. La cantidad disponible descuenta las adjudicaciones ya registradas.</p>
    <button type="button" disabled={busy} onClick={() => {
      if (selected.length === 0 || window.confirm('¿Descartar la selección de esta adjudicación?')) close();
    }}>Volver a adjudicaciones</button>
    <Notice>{error || lot.error}</Notice>
    {valid && <label>Lote<select value={number} disabled={busy} onChange={event => setNumber(Number(event.target.value))}>
      {Array.from({ length: count }, (_, index) => <option key={index + 1} value={index + 1}>Lote {index + 1}</option>)}
    </select></label>}
    {lot.data ? <form onSubmit={event => { event.preventDefault(); void register(); }}>
      <fieldset disabled={busy}>
        <div className="table-wrap"><table className="core-table">
          <thead><tr>{['Seleccionar', 'Ítem', 'Descripción', 'Ofertada', 'Disponible', 'Adjudicar'].map(title => <th key={title}>{title}</th>)}</tr></thead>
          <tbody>{lot.data.items.map(item => {
            const key = `${number}:${item.item_number}`;
            const value = selection[key];
            return <tr key={key}>
              <td><input type="checkbox" aria-label={`Adjudicar ítem ${item.item_number} del lote ${number}`}
                checked={Boolean(value)} disabled={item.remaining_quantity === '0' || (!value && selected.length >= 200)}
                onChange={event => setSelection(current => {
                  const next = { ...current };
                  if (event.target.checked) next[key] = { lot_number: number, item_number: item.item_number, quantity: item.remaining_quantity };
                  else delete next[key];
                  return next;
                })} /></td>
              <td>{item.item_number}</td><td>{item.description}</td><td>{item.offered_quantity} {item.unit}</td>
              <td>{item.remaining_quantity} {item.unit}</td>
              <td>{value && <input aria-label={`Cantidad adjudicada del ítem ${item.item_number} del lote ${number}`}
                inputMode="decimal" required pattern="[0-9]{1,12}([.][0-9]{1,12})?" value={value.quantity}
                onChange={event => setSelection(current => ({ ...current, [key]: { ...value, quantity: event.target.value } }))} />}</td>
            </tr>;
          })}</tbody>
        </table></div>
        <p>{selected.length} ítems seleccionados entre todos los lotes.</p>
        <div className="actions"><button type="button" onClick={() => {
          if (selected.length === 0 || window.confirm('¿Descartar la selección de esta adjudicación?')) close();
        }}>Cancelar</button><button className="primary" disabled={selected.length === 0 || !valid}>
          {busy ? 'Registrando…' : 'Confirmar adjudicación'}
        </button></div>
      </fieldset>
    </form> : !lot.error && valid && <Loading />}
  </DocumentSection>;
}
