export type ConversionModule = 'facturas' | 'ordenes_compra';
export interface ConversionResult { target_module: ConversionModule; target_id: number; quote_id: number; quote_version: number; reused: boolean }
export interface QuoteOrigin { id: number; number: string | null; version: number }
export type OpenConverted = (module: ConversionModule, id: number) => void;
export interface ConversionAttempt { key: string; body: string }

