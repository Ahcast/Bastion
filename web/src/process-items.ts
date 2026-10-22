/** Current Scout numbered text and legacy JSON, always rendered as plain text. */
export function processItems(value: string): string[] {
  if (!value) return [];
  if (value.trimStart().startsWith('[')) {
    try { const parsed: unknown = JSON.parse(value); if (Array.isArray(parsed) && parsed.every(item => typeof item === 'string')) return parsed; } catch { /* Display source as text if an older record is malformed. */ }
  }
  return value.split(/\r?\n/).filter(line => line.trim()).map(line => line.replace(/^\d+\.\s+/, ''));
}
