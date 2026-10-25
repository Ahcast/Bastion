export interface User { id: number; name: string; email: string; role: string }
export interface Permission { module_id: string; scope_key: string; capability: string }
export interface Access { is_system_administrator: boolean; modules: { module_id: string; display_name: string }[]; permissions: Permission[] }
export interface PageResult<T> { items: T[]; page: number; page_size: number }
export interface Person { id: number; full_name: string; dni: string | null; country_code: string | null; gender: string | null }
export interface UserRecord { id: number; person_name: string; email: string; role_name: string; status: string }
export interface Role { id: number; name: string; description: string | null; protected: boolean }
export interface Enrollment { setup_token: string; totp_secret: string; otpauth_uri: string }
export interface Session { id: number; device: string; ip_address: string; started_at: string; last_activity_at: string; current: boolean }
export interface Device { id: number; device: string; ip_address: string; last_used_at: string; expires_at: string }
export interface SecurityEvent { id: number; created_at: string; event_type: string; result: string; ip_address: string; device: string }
export interface Module { module_id: string; display_name: string; included_in_release: boolean; environment_allowed: boolean; enabled: boolean }
export interface Audit { id: number; created_at: string; actor_name: string | null; module_id: string; action: string; target_type: string; target_id: string | number | null; result: string }
export interface Mail { id: number; created_at: string; kind: string; recipient_email: string; delivery_status: string; attempts: number; last_error_code: string | null }
export interface Setting { setting_key: string; value_json: unknown }
export type Page = 'inicio' | 'ajustes' | 'procesos' | 'cotizaciones' | 'clientes' | 'proveedores' | 'ordenes_compra' | 'facturas' | 'gastos' | 'colaboracion';
export function moduleCan(access: Access | null, module: string, scope: string, capability: string): boolean {
  return Boolean(access?.modules.some(m => m.module_id === module) && (access.is_system_administrator || access.permissions.some(p => p.module_id === module && p.capability === capability && (p.scope_key === scope || p.scope_key === '*'))));
}
export function can(access: Access | null, scope: string, capability: string): boolean {
  return Boolean(access?.is_system_administrator || access?.permissions.some(p => p.module_id === 'core' && p.capability === capability && (p.scope_key === scope || p.scope_key === '*')));
}
