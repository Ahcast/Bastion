import { lazy, Suspense, useState } from 'react';
import { ApiError, message } from './api.js';
import type { ApiClient } from './api.js';
import { Brand, Form, Loading, Notice } from './ui.js';
import type { Field } from './ui.js';
import type { Enrollment } from './types.js';
const TotpSetup = lazy(() => import('./components/TotpSetup.js'));
export type AuthFlow = { kind: 'login' | 'totp' | 'recovery' | 'reset' | 'bootstrap' } | { kind: 'activate' | 'reset-complete'; token: string } | { kind: 'enroll'; setup: Enrollment; endpoint: string } | { kind: 'codes'; codes: string[] };
const email: Field = { name: 'email', label: 'Correo', type: 'email', required: true, maxLength: 254, autocomplete: 'username' };
const password: Field = { name: 'password', label: 'Contraseña', type: 'password', required: true, autocomplete: 'current-password' };
const code: Field = { name: 'code', label: 'Código TOTP', required: true, inputMode: 'numeric', autocomplete: 'one-time-code' };
export default function Auth({ api, initial, initialMessage = '', authenticated }: { api: ApiClient; initial: AuthFlow; initialMessage?: string; authenticated: () => Promise<void> }) {
  const [flow, setFlow] = useState<AuthFlow>(initial), [notice, setNotice] = useState(initialMessage);
  const change = (next: AuthFlow) => { setNotice(''); setFlow(next); };
  const enroll = (setup: Enrollment, endpoint: string) => change({ kind: 'enroll', setup, endpoint });
  const back = <button className="ghost" type="button" onClick={() => change({ kind: 'login' })}>Volver al acceso</button>;
  let title = '', description = '', content;
  switch (flow.kind) {
    case 'login':
      title = 'Iniciar sesión'; description = 'Usa el correo y contraseña de tu cuenta.';
      content = <><Form fields={[email, password]} submit="Continuar" onSubmit={async values => {
        const result = await api.request<{ status: string }>('/api/v1/auth/login', { method: 'POST', body: { email: values.email, password: values.password, keep_me_logged_in: values.keep === 'on' } });
        if (result.status === 'totp_required') change({ kind: 'totp' }); else await authenticated();
      }}><label className="checkbox"><input name="keep" type="checkbox" />Mantenerme loggeado</label></Form><div className="auth-links"><button className="text-button" onClick={() => change({ kind: 'reset' })}>Olvidé mi contraseña</button><span>Verificación TOTP</span></div></>;
      break;
    case 'totp': case 'recovery':
      title = flow.kind === 'totp' ? 'Verificación de seguridad' : 'Código de recuperación'; description = flow.kind === 'totp' ? 'Ingresa el código de seis dígitos de tu autenticador.' : 'Usa un código de un solo uso.';
      content = <><Form key={flow.kind} fields={[flow.kind === 'totp' ? code : { name: 'code', label: 'Código de recuperación', required: true, autocomplete: 'one-time-code' }]} submit="Verificar" onSubmit={async values => { await api.request(flow.kind === 'totp' ? '/api/v1/auth/totp' : '/api/v1/auth/recovery-code', { method: 'POST', body: values }); await authenticated(); }} /><div className="auth-links"><button className="text-button" onClick={() => change({ kind: flow.kind === 'totp' ? 'recovery' : 'totp' })}>{flow.kind === 'totp' ? 'Usar código de recuperación' : 'Usar TOTP'}</button><span>Vence en 5 minutos</span></div>{back}</>;
      break;
    case 'reset':
      title = 'Restablecer contraseña'; description = 'Si la cuenta existe, enviaremos instrucciones.';
      content = <><Form fields={[email]} submit="Enviar enlace" onSubmit={async values => { await api.request('/api/v1/auth/password-reset', { method: 'POST', body: values }); setNotice('Si la cuenta está activa, recibirás instrucciones.'); }} />{back}</>;
      break;
    case 'bootstrap':
      title = 'Primera configuración'; description = 'El asistente se desactiva al crear el primer Administrador.';
      content = <Form fields={[{ name: 'bootstrap_secret', label: 'Código secreto temporal', type: 'password', required: true }, { name: 'full_name', label: 'Nombre', maxLength: 180, required: true }, email, { ...password, label: 'Contraseña (8 caracteres o más)', minLength: 8, autocomplete: 'new-password' }]} submit="Crear Administrador" onSubmit={async values => enroll(await api.request<Enrollment>('/api/v1/bootstrap', { method: 'POST', body: values }), '/api/v1/bootstrap/complete')} />;
      break;
    case 'activate':
      title = 'Activar cuenta'; description = 'Define la contraseña y configura TOTP para terminar.';
      content = <Form fields={[{ ...password, label: 'Nueva contraseña', minLength: 8, autocomplete: 'new-password' }]} submit="Continuar" onSubmit={async values => enroll(await api.request<Enrollment>('/api/v1/auth/activate', { method: 'POST', body: { token: flow.token, password: values.password } }), '/api/v1/auth/activate/complete')} />;
      break;
    case 'reset-complete':
      title = 'Nueva contraseña'; description = 'El enlace es de un solo uso y vence en una hora.';
      content = <Form fields={[{ ...password, name: 'new_password', label: 'Nueva contraseña', minLength: 8, autocomplete: 'new-password' }]} submit="Guardar contraseña" onSubmit={async values => { await api.request('/api/v1/auth/password-reset/complete', { method: 'POST', body: { token: flow.token, new_password: values.new_password } }); change({ kind: 'login' }); setNotice('Contraseña actualizada. Inicia sesión con TOTP.'); }} />;
      break;
    case 'enroll':
      title = 'Configura tu autenticador'; description = 'La verificación TOTP es obligatoria.';
      content = <><Suspense fallback={<Loading />}><TotpSetup setup={flow.setup} /></Suspense><Form fields={[code]} submit="Confirmar autenticador" onSubmit={async values => { const result = await api.request<{ recovery_codes: string[] }>(flow.endpoint, { method: 'POST', body: { setup_token: flow.setup.setup_token, code: values.code } }); change({ kind: 'codes', codes: result.recovery_codes }); }} /></>;
      break;
    case 'codes':
      title = 'Guarda tus códigos'; description = 'Cada uno funciona una vez. Guárdalos antes de salir.';
      content = <><pre className="card recovery-codes">{flow.codes.join('\n')}</pre><button className="primary" onClick={() => change({ kind: 'login' })}>Ya los guardé</button></>;
  }
  return <section className="auth-wrap"><div className="auth-card"><Brand /><div className="eyebrow">Acceso al sistema</div><h1>{title}</h1><p className="muted">{description}</p><Notice>{notice}</Notice><div key={flow.kind}>{content}</div></div></section>;
}
export function authFailure(error: unknown) { return error instanceof ApiError && error.status === 401 ? '' : message(error); }
