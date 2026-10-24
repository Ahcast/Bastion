import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Dialog, Loading, Notice, useCore, useLoad } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import type { QuoteSummary } from '../quotes.js';
import type { Destination } from '../orders.js';
import type { ConversionModule, ConversionResult, ConversionAttempt } from '../conversions.js';
export function QuoteConversionDialog({ summary, kind, attempts, close, converted }: {
  summary: QuoteSummary; kind: ConversionModule; attempts: Map<string, ConversionAttempt>; close: () => void; converted: (result: ConversionResult) => void;
}) {
  const { api } = useCore(), { navigate } = useNavigationGuard();
  const slot = `${kind}:${summary.id}:${summary.version}`, prior = attempts.get(slot);
  const initial = prior ? JSON.parse(prior.body) as { n_orders?: string; id_destination?: number } : null;
  const [number, setNumber] = useState(initial?.n_orders ?? ''), [destination, setDestination] = useState(String(initial?.id_destination ?? ''));
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [conflict, setConflict] = useState(false), [ambiguous, setAmbiguous] = useState(Boolean(prior));
  const sending = useRef(false), order = kind === 'ordenes_compra';
  useUnsavedChanges(Boolean(number || destination || ambiguous), busy);
  const prepared = useLoad(async signal => {
    if (!order) return { reason: null as string | null, destinations: [] as Destination[] };
    const [preview, destinations] = await Promise.all([
      api.request<{ quote_version: number; reason: string | null }>(`/api/v1/cotizaciones/${summary.id}/order-conversion`, { signal, ifMatch: summary.version }),
      api.request<Destination[]>('/api/v1/ordenes-compra/destinations', { signal }),
    ]);
    if (preview.quote_version !== summary.version) throw new Error('La cotización cambió. Cierra y abre la versión actual.');
    return { reason: preview.reason, destinations };
  }, [summary.id, summary.version, kind]);
  const blocked = conflict || !prepared.data || Boolean(prepared.data.reason) || (order && !prepared.data.destinations.length);
  return <Dialog title={order ? 'Convertir a orden de compra' : 'Convertir a factura'} close={() => { if (!busy) void navigate(close); }}>
    <p>{summary.quote_number ?? `Cotización ${summary.id}`} · {summary.client_name} · L {summary.total ?? '—'}</p>
    <p>{order ? 'Completa el número de orden y el destino. Se conservarán los productos, precios e impuestos compatibles.' : 'Se creará un borrador a crédito de 30 días con los mismos importes, descuentos e impuestos. No se emitirá una factura fiscal.'}</p>
    <p className="quote-note">Los costos iniciales serán cero. Complétalos en el documento creado.</p>
    <Notice>{error || prepared.error || prepared.data?.reason}</Notice>
    {ambiguous && <p role="status">No se pudo confirmar el guardado. Reintenta los mismos datos para recuperar el resultado sin duplicarlo.</p>}
    {conflict && <p role="status">La cotización cambió. Tus datos se conservan; cierra y consulta su versión actual.</p>}
    {prepared.error && <button type="button" disabled={busy} onClick={prepared.reload}>Reintentar consulta</button>}
    {!prepared.data && !prepared.error && <Loading />}
    {order && prepared.data?.destinations.length === 0 && <p role="status">Agrega un destino en Ajustes &gt; Órdenes de compra antes de convertir.</p>}
    <form className="stack" onSubmit={async event => {
      event.preventDefault(); if (sending.current || blocked) return;
      if (order && (!number.trim() || !destination)) return;
      sending.current = true; setBusy(true); setError('');
      let attempt = attempts.get(slot);
      if (!attempt) { attempt = { key: crypto.randomUUID(), body: JSON.stringify(order ? { n_orders: number, id_destination: Number(destination) } : {}) }; attempts.set(slot, attempt); }
      try {
        const result = await api.request<ConversionResult>(`/api/v1/cotizaciones/${summary.id}/${order ? 'convert-order' : 'convert-invoice'}`, { method: 'POST', ...(order ? { body: JSON.parse(attempt.body) } : {}), ifMatch: summary.version, idempotencyKey: attempt.key });
        attempts.delete(slot); setAmbiguous(false); converted(result);
      } catch (reason) {
        setError(message(reason));
        const certain = reason instanceof ApiError && [400, 403, 404, 409, 412, 422].includes(reason.status);
        if (certain) { attempts.delete(slot); setAmbiguous(false); if (reason.status === 412) setConflict(true); }
        else setAmbiguous(true);
      } finally { sending.current = false; setBusy(false); }
    }}>
      {order && <fieldset disabled={busy || ambiguous || blocked} className="form-fields">
        <label>Número de orden<input required maxLength={50} value={number} onChange={event => setNumber(event.target.value)} /></label>
        <label>Destino<select required value={destination} onChange={event => setDestination(event.target.value)}><option value="">Selecciona un destino</option>{prepared.data?.destinations.map(item => <option key={item.id} value={item.id}>{item.destination}</option>)}</select></label>
      </fieldset>}
      <div className="actions"><button type="button" disabled={busy} onClick={() => void navigate(close)}>Cancelar</button><button className="primary" type="submit" disabled={busy || blocked || (order && (!number.trim() || !destination))}>{busy ? 'Convirtiendo…' : ambiguous ? 'Reintentar conversión' : order ? 'Crear orden de compra' : 'Crear borrador de factura'}</button></div>
    </form>
  </Dialog>;
}

