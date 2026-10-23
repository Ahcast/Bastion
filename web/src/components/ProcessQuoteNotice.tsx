import type { ProcessQuoteSeed } from '../processQuote.js';

export function ProcessQuoteNotice({ seed, missingQuantities }: { seed: ProcessQuoteSeed; missingQuantities: number }) {
  return <aside className="quote-source-notice" role="status" aria-live="polite">
    <p>El listado de ítems se generó automáticamente. Revisa siempre el proceso original antes de cotizar.
      {' '}{seed.sourceUrl ? <a href={seed.sourceUrl} target="_blank" rel="noopener noreferrer">Ver proceso {seed.expediente} ↗</a> : <span>Expediente: {seed.expediente}.</span>}
    </p>
    {missingQuantities > 0 && <p>{missingQuantities} {missingQuantities === 1 ? 'ítem sin cantidad verificable' : 'ítems sin cantidad verificable'}. Completa las cantidades con el proceso original.</p>}
  </aside>;
}
