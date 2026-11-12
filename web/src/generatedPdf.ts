import type { ApiClient } from './api.js';

export function reserveGeneratedPdfViewer(api: Pick<ApiClient, 'downloadHref'>) {
  // Reserve during the click, including when drafts must be saved before navigation.
  const viewer = window.open('about:blank', '_blank');
  if (!viewer) throw new Error('El navegador bloqueó la pestaña del PDF. Permite abrirla y vuelve a intentarlo.');
  viewer.opener = null;
  viewer.document.title = 'Preparando PDF';
  if (viewer.document.body) viewer.document.body.textContent = 'Preparando PDF…';
  return {
    show(path: string) {
      if (viewer.closed) return;
      try { if (viewer.location.href !== 'about:blank') return; } catch { return; }
      // Navigate to the authenticated PDF response so the native viewer receives
      // Content-Disposition and its filename, rather than an anonymous Blob URL.
      viewer.location.replace(api.downloadHref(path));
    },
    close() {
      try { if (!viewer.closed && viewer.location.href === 'about:blank') viewer.close(); } catch { /* User navigated elsewhere. */ }
    },
  };
}

export function openGeneratedPdf(api: Pick<ApiClient, 'downloadHref'>, path: string) {
  const viewer = reserveGeneratedPdfViewer(api);
  try { viewer.show(path); } catch (reason) { viewer.close(); throw reason; }
}
