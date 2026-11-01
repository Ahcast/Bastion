/** Keep OAuth parameters in memory and remove them from browser history. */
export function takeMcpQuery(enabled: boolean, current: { pathname: string; search: string; hash?: string }, replace: (path: string) => void): string | null {
  if (!enabled || current.pathname !== '/oauth/consent') return null;
  const fragment = current.hash?.startsWith('#') ? current.hash.slice(1) : (current.hash ?? '');
  const search = current.search.startsWith('?') ? current.search.slice(1) : current.search;
  replace(current.pathname);
  if (fragment && search) return '';
  const query = fragment || search;
  return query.length <= 8192 ? query : '';
}
