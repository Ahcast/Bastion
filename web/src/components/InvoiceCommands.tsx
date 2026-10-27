import { useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Dialog, Notice, useCore } from '../ui.js';
import { Decimal } from './QuoteItems.js';
import { newInvoice } from '../invoices.js';
import type { InvoiceDraft, InvoiceSummary, PaymentInput } from '../invoices.js';

export const newPayment = (): PaymentInput => ({ amount: '0', date: newInvoice().invoice_date, method: '', reference: '' });
export function InvoicePaymentFields({ value, change }: { value: PaymentInput; change: (value: PaymentInput) => void }) {
  return <div className="form-fields"><Decimal label="Monto del pago (HNL)" value={value.amount} change={amount => change({ ...value, amount })} /><label>Fecha del pago<input name="payment_date" type="date" min="1000-01-01" max="9999-12-31" required value={value.date} onChange={event => change({ ...value, date: event.target.value })} /></label><label>Método de pago<input name="payment_method" maxLength={120} value={value.method} onChange={event => change({ ...value, method: event.target.value })} /></label><label>Referencia del pago<input name="payment_reference" maxLength={180} value={value.reference} onChange={event => change({ ...value, reference: event.target.value })} /></label></div>;
}

export function InvoiceCommandDialog({ invoice, action, close, completed, changed }: { invoice: InvoiceSummary; action: 'sent' | 'payment' | 'delete'; close: () => void; completed: (result: InvoiceDraft | null) => void; changed: () => void }) {
  const { api } = useCore();
  const [payment, setPayment] = useState(newPayment), [busy, setBusy] = useState(false), [error, setError] = useState(''), [ambiguous, setAmbiguous] = useState(false), [conflict, setConflict] = useState(false);
  const key = useRef(crypto.randomUUID());
  const title = action === 'sent' ? 'Marcar como enviado' : action === 'payment' ? 'Ingresar pago' : 'Eliminar factura';
  return <Dialog title={`${title} · ${invoice.invoice_number ?? `Borrador ${invoice.id}`}`} close={() => { if (!busy) close(); }}>
    {action === 'sent' && <p>Se asignará el siguiente número SAR y la factura quedará bloqueada para edición y eliminación. Esta acción no envía un correo. Revisa empresa, rango autorizado y fecha límite en Ajustes antes de confirmar.</p>}
    {action === 'delete' && <p>Se quitará el borrador del listado. Sus relaciones de origen se conservarán; no se podrá repetir su conversión ni recuperar el borrador desde esta pantalla. Solo se permiten borradores sin pagos.</p>}
    {action === 'payment' && <p>Saldo registrado: L {invoice.balance}. El pago no puede superar este saldo.</p>}
    <Notice>{error}</Notice>{ambiguous && <p role="status">No se confirmó la respuesta. Reintenta con los mismos datos y la misma clave para evitar duplicados.</p>}
    <form className="stack" onSubmit={async event => {
      event.preventDefault(); if (busy || conflict) return; setBusy(true); setError('');
      try {
        const result = await api.request<InvoiceDraft>(`/api/v1/facturas/${invoice.id}${action === 'sent' ? '/mark-sent' : action === 'payment' ? '/payments' : ''}`, { method: action === 'delete' ? 'DELETE' : 'POST', body: action === 'payment' ? payment : undefined, ifMatch: invoice.version, idempotencyKey: key.current });
        completed(action === 'delete' ? null : result);
      } catch (reason) {
        setError(reason instanceof ApiError && reason.status === 412 ? 'La factura cambió. Cierra esta ventana y consulta la versión actual antes de continuar.' : action === 'sent' && reason instanceof ApiError && [400,409,422].includes(reason.status) ? 'No se pudo emitir. Revisa los datos de la factura, el pago de contado y la configuración de Empresa/SAR (número actual, rango completo y fecha límite vigente).' : message(reason));
        if (reason instanceof ApiError && reason.status === 412) {setConflict(true);changed();}
        if (!(reason instanceof ApiError) || reason.status >= 500) setAmbiguous(true);
      } finally { setBusy(false); }
    }}><fieldset disabled={busy || ambiguous || conflict} className="stack">{action === 'payment' && <InvoicePaymentFields value={payment} change={value => { setPayment(value); key.current = crypto.randomUUID(); }} />}</fieldset>
      <div className="actions"><button disabled={busy} type="button" onClick={close}>Cerrar</button><button disabled={busy || conflict} type="submit" className={action === 'delete' ? 'danger' : 'primary'}>{busy ? 'Procesando…' : ambiguous ? 'Reintentar la misma operación' : action === 'payment' ? 'Guardar pago' : action === 'sent' ? 'Confirmar y asignar número SAR' : 'Eliminar borrador'}</button></div>
    </form>
  </Dialog>;
}
