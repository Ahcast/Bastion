import { useRef, useState } from 'react';
import { message } from '../api.js';
import { Notice, useCore } from '../ui.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { reservePdfViewer } from '../quoteDocuments.js';
import { maxTenderFileBytes, tenderContentBlob } from '../tenderAttachments.js';
import type { FileContent } from '../collaboration.js';
import type { Expedient } from '../tenders.js';

export default function TenderEconomicPreview({ latest }: { latest: () => Expedient }) {
  const { api } = useCore();
  const { saveAll, saveError } = useNavigationGuard();
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  const working = useRef(false);
  useUnsavedChanges(false, busy);
  return <div className="stack"><div className="actions"><button type="button" disabled={busy} onClick={async () => {
    if (working.current) return;
    working.current = true; setError('');
    let viewer: ReturnType<typeof reservePdfViewer> | undefined;
    try {
      viewer = reservePdfViewer(maxTenderFileBytes);
      if (!await saveAll()) { viewer.close(); return; }
      setBusy(true);
      const record = latest();
      const content = await api.request<FileContent>(`/api/v1/licitaciones/${record.id}/oferta-economica/preview`, { method:'POST', ifMatch:record.version, body:{} });
      tenderContentBlob(content, true);
      viewer.show(content);
    } catch (reason) { viewer?.close(); setError(message(reason)); }
    finally { working.current = false; setBusy(false); }
  }}>{busy ? 'Generando vista previa…' : 'Vista previa de oferta económica'}</button></div><p className="document-note">Guarda los cambios y abre la carta y los lotes con ítems. El total de páginas corresponde solo a esta vista previa.</p><Notice>{error || saveError}</Notice></div>;
}
