/** Remove OAuth parameters from browser history before loading authenticated UI. */
export function takeMcpQuery(enabled: boolean, current: { pathname: string; search: string }, replace: (path: string) => void): string | null {
  if (!enabled || current.pathname !== '/oauth/consent') return null;
  const query = current.search.startsWith('?') ? current.search.slice(1) : current.search;
  replace(current.pathname);
  return query.length <= 8192 ? query : '';
}
