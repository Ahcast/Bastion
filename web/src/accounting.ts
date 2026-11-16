export interface Account { code: string; name: string; class: string; parent_code: string | null; posting: boolean; active: boolean; version: number }
export interface Period { month: string; closed: boolean; version: number }
export interface SourceRef { module: string; id: number; version: number }
export interface SourceSnapshot { reference: SourceRef; label: string; amount: string; date: string; tax: string }
export interface EntryLine { account: string; debit: string; credit: string }
export interface EntryInput { date: string; description: string; reference: string; kind: string; source: SourceRef | null; lines: EntryLine[] }
export interface Entry extends EntryInput { id: number; version: number; status: string; total: string; reversal_of: number | null; reversed_by: number | null; source_snapshot: SourceSnapshot | null; source_status: string }
export interface EntrySummary { id: number; version: number; date: string; description: string; status: string; kind: string; total: string; reversed_by: number | null }
export interface EntryPage { items: EntrySummary[]; page: number; has_more: boolean }
export interface Proposal { entry: EntryInput; source: SourceSnapshot; notice: string }
export interface Balance { date_from: string; date_to: string; currency: string; items: { code: string; name: string; class: string; opening: string; debit: string; credit: string; balance: string }[]; total_debit: string; total_credit: string; balanced: boolean }
export const accountingPath = '/api/v1/contabilidad';
export const accountingToday = () => new Intl.DateTimeFormat('sv-SE', { timeZone: 'America/Tegucigalpa', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date());
export const newEntry = (): EntryInput => ({ date: accountingToday(), description: '', reference: '', kind: 'manual', source: null, lines: [{ account: '', debit: '0.00', credit: '0.00' }, { account: '', debit: '0.00', credit: '0.00' }] });
export const entryInput = (e: EntryInput): EntryInput => ({ date: e.date, description: e.description, reference: e.reference, kind: e.kind, source: e.source, lines: e.lines.map(l => ({ account: l.account, debit: l.debit, credit: l.credit })) });
export function exactCents(value: string): bigint | null { if (!/^\d{1,12}(?:\.\d{1,2})?$/.test(value)) return null; const [whole = '0', decimals = ''] = value.split('.'); return BigInt(whole) * 100n + BigInt(decimals.padEnd(2, '0')); }
export function formatCents(value: bigint) { const sign = value < 0 ? '-' : ''; const n = value < 0 ? -value : value; return `${sign}${n / 100n}.${String(n % 100n).padStart(2, '0')}`; }
export function entryTotals(lines: EntryLine[]) { let debit = 0n, credit = 0n, valid = lines.length >= 2 && lines.length <= 100; for (const line of lines) { const d = exactCents(line.debit), c = exactCents(line.credit); if (d === null || c === null || !line.account || (d > 0) === (c > 0)) valid = false; debit += d ?? 0n; credit += c ?? 0n; } return { debit: formatCents(debit), credit: formatCents(credit), difference: formatCents(debit - credit), valid: valid && debit === credit && debit > 0 && debit <= 99999999999999n }; }
