export interface ClientInput {
  cliente_name: string;
  user_contact: string;
  email: string;
  rtn: string;
  phone_number: string;
  mobile_number: string;
  direction: string;
}
export interface Client extends ClientInput { id: number; version: number }
export interface ClientPage<T = Client> { items: T[]; page: number; page_size: number; has_more: boolean }
export type CatalogClient = Pick<Client, 'id' | 'cliente_name' | 'email' | 'rtn'>;
export interface ClientWrite { client: Client; merged: boolean }
export const emptyClient = (): ClientInput => ({ cliente_name: '', user_contact: '', email: '', rtn: '', phone_number: '', mobile_number: '', direction: '' });
export const clientInput = (client: ClientInput): ClientInput => ({ cliente_name: client.cliente_name, user_contact: client.user_contact, email: client.email, rtn: client.rtn, phone_number: client.phone_number, mobile_number: client.mobile_number, direction: client.direction });
