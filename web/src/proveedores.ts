export interface SupplierInput {
  supplier_name: string;
  user_contact: string;
  email: string;
  rtn: string;
  phone_number: string;
  mobile_number: string;
  direction: string;
}

export interface Supplier extends SupplierInput { id: number; version: number }
export interface CatalogSupplier { id: number; supplier_name: string }
export interface SupplierPage<T = Supplier> { items: T[]; page: number; page_size: number; has_more: boolean }

export const emptySupplier = (): SupplierInput => ({
  supplier_name: '', user_contact: '', email: '', rtn: '', phone_number: '', mobile_number: '', direction: '',
});

export const supplierInput = (supplier: SupplierInput): SupplierInput => ({
  supplier_name: supplier.supplier_name,
  user_contact: supplier.user_contact,
  email: supplier.email,
  rtn: supplier.rtn,
  phone_number: supplier.phone_number,
  mobile_number: supplier.mobile_number,
  direction: supplier.direction,
});
