export interface ExpenseInput {
  expense_date: string;
  invoice_number: string;
  provider: string;
  currency: 'HNL';
  taxable_amount: string;
  exempt_amount: string;
}

export interface Expense extends ExpenseInput {
  id: number;
  version: number;
  isv_15: string;
  total: string;
}

export interface ExpensePage { items: Expense[]; page: number; page_size: number; has_more: boolean }

export const emptyExpense = (expense_date: string): ExpenseInput => ({
  expense_date, invoice_number: '', provider: '', currency: 'HNL', taxable_amount: '0', exempt_amount: '0',
});

export const expenseInput = (expense: ExpenseInput): ExpenseInput => ({ ...expense });
