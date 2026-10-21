export class ApiError extends Error {
  readonly status: number;
  readonly title: string;
  constructor(status: number, title: string, detail: string) { super(detail); this.status = status; this.title = title; }
}
type Options = { method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'; body?: unknown; signal?: AbortSignal };
type Hooks = { unauthorized?: () => void; reauthenticate?: () => void };
const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
export class ApiClient {
  private csrf = '';
  private refreshing: Promise<void> | null = null;
  hooks: Hooks = {};
  constructor(private readonly base: string, private readonly transport: typeof fetch = fetch) {}
  async refreshCsrf(): Promise<void> {
    if (!this.refreshing) {
      this.refreshing = this.request<{ csrf_token: string }>('/api/v1/auth/csrf').then(() => {}).finally(() => { this.refreshing = null; });
    }
    return this.refreshing;
  }
  async request<T = void>(path: string, options: Options = {}, retried = false): Promise<T> {
    const method = options.method ?? 'GET';
    const headers = new Headers();
    if (options.body !== undefined) headers.set('Content-Type', 'application/json');
    if (method !== 'GET') headers.set('X-CSRF-Token', this.csrf);
    let response: Response;
    try {
      const transport = this.transport;
      response = await transport(this.base + path, { method, headers, credentials: 'include', cache: 'no-store', signal: options.signal, body: options.body === undefined ? undefined : JSON.stringify(options.body) });
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') throw error;
      throw new Error('No se pudo conectar a la API; comprueba la conexión y vuelve a intentarlo.');
    }
    const data: unknown = response.status === 204 ? null : await response.json().catch(() => null);
    if (record(data) && typeof data.csrf_token === 'string') this.csrf = data.csrf_token;
    if (!response.ok) {
      const title = record(data) && typeof data.title === 'string' ? data.title : '';
      if (method !== 'GET' && response.status === 403 && title === 'CSRF Validation Failed') {
        if (!retried) { await this.refreshCsrf(); return this.request<T>(path, options, true); }
        this.hooks.unauthorized?.();
        throw new ApiError(403, title, 'La verificación de seguridad venció. Inicia sesión de nuevo.');
      }
      if (response.status === 401) this.hooks.unauthorized?.();
      if (response.status === 428) this.hooks.reauthenticate?.();
      throw new ApiError(response.status, title, record(data) && typeof data.detail === 'string' ? data.detail : 'La solicitud no se pudo completar.');
    }
    return data as T;
  }
}
export function apiOrigin(value: unknown): string {
  if (typeof value !== 'string') throw new Error('Configura el origen público del API en config.js.');
  const url = new URL(value);
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname))) throw new Error('La API debe usar HTTPS.');
  if (url.username || url.password || url.search || url.hash || url.pathname !== '/') throw new Error('Configura solo el origen del API, sin rutas ni credenciales.');
  return url.origin;
}
export const message = (error: unknown): string => error instanceof Error ? error.message : 'La solicitud no se pudo completar.';
