package invoices

import (
	"bytes"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"strings"
	"testing"
	"time"
)

func documentTestInput(currency string) Input {
	days := uint16(30)
	return Input{
		Customer:    CustomerInput{ID: "44", Name: "Cliente Histórico, S.A.", RTN: "08011999123456", Contact: "Ana López", Phone: "+504 2200-0000", Email: "ana@example.test", Address: "Tegucigalpa, Honduras"},
		InvoiceDate: "2026-10-10", PaymentTerm: "credito", CreditDays: &days, Currency: currency,
		PublicNotes: "Entrega coordinada con el cliente.", DiscountType: "amount", Discount: "20.00", Exoneration: "EXO-2026-TEST",
		AdditionalExpenses: "17.00", TeamID: "22",
		Items: []ItemInput{
			{ID: "101", Item: finance.Item{Description: "Equipo gravado 15%", Unit: "unidad", Quantity: "2", Price: "100.00", Cost: "51.123456", DiscountType: "percent", Discount: "5", Tax: "isv15"}},
			{ID: "102", Item: finance.Item{Description: "Equipo gravado 18%", Unit: "unidad", Quantity: "1.5", Price: "200.00", Cost: "93.654321", DiscountType: "amount", Discount: "10.00", Tax: "isv18"}},
			{ID: "103", Item: finance.Item{Description: "Servicio exento", Unit: "servicio", Quantity: "3", Price: "25.50", Cost: "5.00", DiscountType: "amount", Discount: "0", Tax: "exempt"}},
			{ID: "104", Item: finance.Item{Description: "Bien exonerado", Unit: "unidad", Quantity: "1", Price: "50.00", Cost: "10.00", DiscountType: "percent", Discount: "0", Tax: "exonerated"}},
		},
	}
}

func documentTestInvoice(t *testing.T, currency string) Invoice {
	t.Helper()
	p := documentTestInput(currency)
	result, e := Validate(p)
	if e != nil {
		t.Fatal(e)
	}
	o := Invoice{
		ID: "501", Status: "draft", Customer: p.Customer, CustomerVersion: 8,
		InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, CreditDays: p.CreditDays, Currency: p.Currency,
		PublicNotes: p.PublicNotes, DiscountType: p.DiscountType, Discount: p.Discount, Exoneration: p.Exoneration,
		Calculation: result.Calculation, PaidTotal: "0.00", Balance: result.Calculation.Total,
		Version: 7, CreatedBy: "11", TeamID: p.TeamID,
	}
	for i, x := range result.Items {
		o.Items = append(o.Items, Item{ID: p.Items[i].ID, CalculatedItem: x})
	}
	return o
}

func issueDocumentTestInvoice(t *testing.T, o Invoice) Invoice {
	t.Helper()
	company, e := json.Marshal(core.CompanySettings{Name: "Castillo IT Histórico", TaxID: "08019000000001", Address: "Tegucigalpa, Honduras"})
	if e != nil {
		t.Fatal(e)
	}
	fiscal := FiscalSnapshot{Configuration: FiscalAuthorization{
		CAI: "CAI-TEST-2026", Prefix: "000-001-01-", Width: 8, RangeStart: 1, RangeEnd: 99999999,
		ValidFrom: "2026-01-01", ValidUntil: "2026-12-31", Currency: o.Currency, Enabled: true,
	}, Version: 5}
	sar, e := json.Marshal(fiscal)
	if e != nil {
		t.Fatal(e)
	}
	number := "000-001-01-00000042"
	issuedBy := "11"
	due, e := invoiceDueDate(o)
	if e != nil {
		t.Fatal(e)
	}
	issuedAt := time.Date(2026, 10, 10, 18, 0, 0, 0, time.UTC)
	companyVersion := uint64(3)
	o.Status = "issued"
	o.Number = &number
	o.SAR = sar
	o.IssuedAt = &issuedAt
	o.IssuedBy = &issuedBy
	o.DueDate = &due
	o.Company = company
	o.CompanyVersion = &companyVersion
	return o
}

func documentDraftContext() *DraftDocumentContext {
	return &DraftDocumentContext{Company: core.CompanySettings{Name: "Castillo IT Actual", TaxID: "08019000000999", Address: "Dirección actual"}, CompanyVersion: 99}
}

func documentTestPayment(o Invoice) Payment {
	return Payment{
		ID: "701", InvoiceID: o.ID, Date: "2026-10-11", Method: "Transferencia", Currency: o.Currency,
		Amount: "50.00", Reference: "TRX-TEST-701", Reason: "razón privada de cobro", Version: 1,
		CreatedBy: "11", CreatedAt: time.Date(2026, 10, 11, 15, 0, 0, 0, time.UTC),
	}
}

func assertJSONDoesNotContain(t *testing.T, value any, fragments ...string) string {
	t.Helper()
	raw, e := json.Marshal(value)
	if e != nil {
		t.Fatal(e)
	}
	s := string(raw)
	for _, fragment := range fragments {
		if strings.Contains(s, fragment) {
			t.Fatalf("public document leaked %q: %s", fragment, s)
		}
	}
	return s
}

func TestProjectDocumentPrivacyAndHistoricalSnapshots(t *testing.T) {
	o := issueDocumentTestInvoice(t, documentTestInvoice(t, "HNL"))
	current := documentDraftContext()

	invoice, e := ProjectDocument(DocumentInvoice, o, nil, current)
	if e != nil {
		t.Fatal(e)
	}
	if invoice.Company.Name != "Castillo IT Histórico" || invoice.CompanyVersion != 3 || invoice.Fiscal == nil || invoice.Fiscal.CAI != "CAI-TEST-2026" || invoice.InvoiceNumber != "000-001-01-00000042" {
		t.Fatalf("issued projection did not preserve historical snapshots: %#v", invoice)
	}
	if invoice.Company.Name == current.Company.Name || invoice.CompanyVersion == current.CompanyVersion {
		t.Fatal("issued document used current draft context")
	}
	public := assertJSONDoesNotContain(t, invoice,
		`"id":`, `"unit_cost":`, `"cost":`, `"products_investment":`, `"additional_expenses":`,
		`"investment_total":`, `"profit":`, `"created_by":`, `"team_id":`, `"source_quote_id":`,
		`"source_purchase_order_id":`, `"paid_total":`, `"balance":`, `"reason":`, `"void_reason":`)
	for _, required := range []string{`"unit_price":`, `"base_15":`, `"base_18":`, `"exempt":`, `"exonerated":`, `"isv_15":`, `"isv_18":`, `"total":`} {
		if !strings.Contains(public, required) {
			t.Fatalf("invoice public projection missing %s", required)
		}
	}

	delivery, e := ProjectDocument(DocumentDeliveryNote, o, nil, current)
	if e != nil {
		t.Fatal(e)
	}
	if delivery.Company.Name != "Castillo IT Histórico" || delivery.Delivery == nil || delivery.InvoiceNumber == "" {
		t.Fatal("delivery note lost historical identity")
	}
	assertJSONDoesNotContain(t, delivery, `"currency":`, `"unit_price":`, `"unit_discount":`, `"item_discount":`, `"general_discount":`, `"base":`, `"tax":`, `"totals":`, `"fiscal":`, `"cai":`, `"paid_total":`, `"balance":`)

	payment := documentTestPayment(o)
	receipt, e := ProjectDocument(DocumentReceipt, o, &payment, current)
	if e != nil {
		t.Fatal(e)
	}
	if receipt.Receipt == nil || receipt.Receipt.Number != "RCP-701" || receipt.Receipt.Amount != "50.00" || receipt.Receipt.Voided {
		t.Fatalf("unexpected receipt projection: %#v", receipt.Receipt)
	}
	assertJSONDoesNotContain(t, receipt, `"totals":`, `"paid_total":`, `"balance":`, `"fiscal":`, `"cai":`, `"reason":`, `"void_reason":`, `"receipt_number":`, `"invoice_id":`)
}

func TestProjectDraftNeverLooksFiscal(t *testing.T) {
	o := documentTestInvoice(t, "USD")
	doc, e := ProjectDocument(DocumentInvoice, o, nil, documentDraftContext())
	if e != nil {
		t.Fatal(e)
	}
	if doc.Label != "Borrador - sin validez fiscal" || doc.InvoiceNumber != "" || doc.Fiscal != nil || doc.Company.Name != "Castillo IT Actual" || doc.CompanyVersion != 99 || doc.DueDate != "2026-11-09" {
		t.Fatalf("bad draft public projection: %#v", doc)
	}
	if _, e = ProjectDocument(DocumentInvoice, o, nil, nil); e != core.Invalid {
		t.Fatalf("missing draft context: got %v", e)
	}
	if _, e = ProjectDocument(DocumentDeliveryNote, o, nil, documentDraftContext()); e != core.Conflict {
		t.Fatalf("draft delivery note: got %v", e)
	}
	if _, e = ProjectDocument(DocumentKind("other"), o, nil, documentDraftContext()); e != core.Invalid {
		t.Fatalf("unknown document kind: got %v", e)
	}
}

func TestCancelledInvoiceDocumentIsMarkedAndHasNoDeliveryOrReceipt(t *testing.T) {
	o := issueDocumentTestInvoice(t, documentTestInvoice(t, "HNL"))
	now := time.Date(2026, 10, 12, 10, 0, 0, 0, time.UTC)
	by := "11"
	o.Status, o.CancelledAt, o.CancelledBy = "cancelled", &now, &by
	doc, e := ProjectDocument(DocumentInvoice, o, nil, nil)
	if e != nil || doc.Label != "Factura anulada" || doc.Fiscal == nil {
		t.Fatal(doc, e)
	}
	if _, _, e = RenderDocumentPDF(doc); e != nil {
		t.Fatal(e)
	}
	if _, e = ProjectDocument(DocumentDeliveryNote, o, nil, nil); e != core.Conflict {
		t.Fatal(e)
	}
	if _, e = ProjectDocument(DocumentReceipt, o, nil, nil); e != core.Conflict {
		t.Fatal(e)
	}
}

func TestProjectReceiptVoidedAndIncoherentPayments(t *testing.T) {
	o := issueDocumentTestInvoice(t, documentTestInvoice(t, "HNL"))
	payment := documentTestPayment(o)
	voidedAt := time.Date(2026, 10, 12, 12, 0, 0, 0, time.UTC)
	voidedBy := "12"
	voidReason := "duplicado interno que no debe publicarse"
	payment.VoidedAt = &voidedAt
	payment.VoidedBy = &voidedBy
	payment.VoidReason = &voidReason
	payment.Version = 2
	doc, e := ProjectDocument(DocumentReceipt, o, &payment, nil)
	if e != nil {
		t.Fatal(e)
	}
	if doc.Label != "Recibo anulado" || doc.Receipt == nil || !doc.Receipt.Voided || doc.Receipt.Amount != "50.00" {
		t.Fatalf("bad voided receipt: %#v", doc)
	}
	assertJSONDoesNotContain(t, doc, "duplicado interno", "razón privada", `"void_reason":`, `"reason":`)

	bad := payment
	bad.InvoiceID = "999"
	if _, e = ProjectDocument(DocumentReceipt, o, &bad, nil); e != core.Conflict {
		t.Fatalf("foreign payment: got %v", e)
	}
	bad = payment
	bad.Currency = "USD"
	if _, e = ProjectDocument(DocumentReceipt, o, &bad, nil); e != core.Conflict {
		t.Fatalf("currency mismatch: got %v", e)
	}
	bad = payment
	bad.Amount = "0.00"
	if _, e = ProjectDocument(DocumentReceipt, o, &bad, nil); e != core.Conflict {
		t.Fatalf("zero receipt: got %v", e)
	}
	if _, e = ProjectDocument(DocumentReceipt, o, nil, nil); e != core.Invalid {
		t.Fatalf("missing receipt context: got %v", e)
	}
}

func TestProjectDocumentRejectsCorruptHistoricalState(t *testing.T) {
	o := issueDocumentTestInvoice(t, documentTestInvoice(t, "HNL"))
	corrupt := o
	corrupt.Calculation.Total = "1.00"
	if _, e := ProjectDocument(DocumentInvoice, corrupt, nil, nil); e != core.Conflict {
		t.Fatalf("corrupt calculation: got %v", e)
	}
	corrupt = o
	corrupt.Company = json.RawMessage(`{"name":"Old","tax_id":"1","address":"x","private":"leak"}`)
	if _, e := ProjectDocument(DocumentInvoice, corrupt, nil, nil); e != core.Conflict {
		t.Fatalf("unknown historical company field: got %v", e)
	}
	corrupt = o
	wrong := "000-001-01-999999999"
	corrupt.Number = &wrong
	if _, e := ProjectDocument(DocumentInvoice, corrupt, nil, nil); e != core.Conflict {
		t.Fatalf("incoherent fiscal number: got %v", e)
	}
}

func largeExactInvoice(t *testing.T, currency string) Invoice {
	t.Helper()
	p := Input{
		Customer: CustomerInput{Name: "Cliente exacto"}, InvoiceDate: "2026-10-10", PaymentTerm: "contado", Currency: currency,
		DiscountType: "amount", Discount: "0.00", AdditionalExpenses: "0.00", TeamID: "22",
		Items: []ItemInput{{ID: "201", Item: finance.Item{Description: "Valor mayor a 2^53", Unit: "unidad", Quantity: "10000", Price: "900719925474.0993", Cost: "0.000001", DiscountType: "amount", Discount: "0", Tax: "exempt"}}},
	}
	result, e := Validate(p)
	if e != nil {
		t.Fatal(e)
	}
	return Invoice{ID: "502", Status: "draft", Customer: p.Customer, InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, Currency: p.Currency, DiscountType: p.DiscountType, Discount: p.Discount, Calculation: result.Calculation, PaidTotal: "0.00", Balance: result.Calculation.Total, Version: 1, CreatedBy: "11", TeamID: p.TeamID, Items: []Item{{ID: "201", CalculatedItem: result.Items[0]}}}
}

func TestPublicDocumentPreservesExactLargeDecimalsAndDeterministicPDF(t *testing.T) {
	for _, currency := range []string{"HNL", "USD"} {
		t.Run(currency, func(t *testing.T) {
			o := largeExactInvoice(t, currency)
			doc, e := ProjectDocument(DocumentInvoice, o, nil, documentDraftContext())
			if e != nil {
				t.Fatal(e)
			}
			if doc.Invoice.Totals.Total != "9007199254740993.00" || doc.Invoice.Items[0].UnitPrice != "900719925474.0993" {
				t.Fatalf("precision changed: total=%s price=%s", doc.Invoice.Totals.Total, doc.Invoice.Items[0].UnitPrice)
			}
			a, pagesA, e := RenderDocumentPDF(doc)
			if e != nil {
				t.Fatal(e)
			}
			b, pagesB, e := RenderDocumentPDF(doc)
			if e != nil {
				t.Fatal(e)
			}
			if pagesA != pagesB || pagesA < 1 || !bytes.Equal(a, b) {
				t.Fatalf("renderer not deterministic: pages %d/%d bytes %d/%d", pagesA, pagesB, len(a), len(b))
			}
		})
	}
}

func longDocumentInvoice(t *testing.T) Invoice {
	t.Helper()
	p := Input{
		Customer:    CustomerInput{Name: "Cliente de paginación", RTN: "08010000000000", Address: "Dirección con acentos: información pública"},
		InvoiceDate: "2026-10-10", PaymentTerm: "contado", Currency: "HNL",
		PublicNotes: strings.Repeat("Información pública con áéíóú y ñ. ", 200), DiscountType: "amount", Discount: "0.00", AdditionalExpenses: "0.00", TeamID: "22",
	}
	for i := 0; i < 80; i++ {
		p.Items = append(p.Items, ItemInput{ID: fmt.Sprint(300 + i), Item: finance.Item{Description: fmt.Sprintf("Artículo %d %s", i+1, strings.Repeat("descripciónlarga", 12)), Unit: "unidad", Quantity: "1", Price: "1.25", Cost: "0.25", DiscountType: "amount", Discount: "0", Tax: "isv15"}})
	}
	result, e := Validate(p)
	if e != nil {
		t.Fatal(e)
	}
	o := Invoice{ID: "503", Status: "draft", Customer: p.Customer, InvoiceDate: p.InvoiceDate, PaymentTerm: p.PaymentTerm, Currency: p.Currency, PublicNotes: p.PublicNotes, DiscountType: p.DiscountType, Discount: p.Discount, Calculation: result.Calculation, PaidTotal: "0.00", Balance: result.Calculation.Total, Version: 1, CreatedBy: "11", TeamID: p.TeamID}
	for i, x := range result.Items {
		o.Items = append(o.Items, Item{ID: p.Items[i].ID, CalculatedItem: x})
	}
	return o
}

func TestRenderDocumentUnicodeWrappingPaginationAndMissingGlyph(t *testing.T) {
	doc, e := ProjectDocument(DocumentInvoice, longDocumentInvoice(t), nil, documentDraftContext())
	if e != nil {
		t.Fatal(e)
	}
	data, pages, e := RenderDocumentPDF(doc)
	if e != nil {
		t.Fatal(e)
	}
	if len(data) == 0 || pages < 2 || pages > 250 {
		t.Fatalf("expected paginated PDF, got pages=%d bytes=%d", pages, len(data))
	}
	bad := doc
	bad.PublicNotes = "Glifo no integrado 😀"
	if _, _, e = RenderDocumentPDF(bad); e != core.Invalid {
		t.Fatalf("missing glyph must fail validation, got %v", e)
	}
}

func TestRenderAllPublicDocumentKinds(t *testing.T) {
	o := issueDocumentTestInvoice(t, documentTestInvoice(t, "HNL"))
	payment := documentTestPayment(o)
	for _, tc := range []struct {
		kind    DocumentKind
		payment *Payment
	}{
		{DocumentInvoice, nil},
		{DocumentDeliveryNote, nil},
		{DocumentReceipt, &payment},
	} {
		doc, e := ProjectDocument(tc.kind, o, tc.payment, documentDraftContext())
		if e != nil {
			t.Fatalf("project %s: %v", tc.kind, e)
		}
		data, pages, e := RenderDocumentPDF(doc)
		if e != nil {
			t.Fatalf("render %s: %v", tc.kind, e)
		}
		if len(data) == 0 || pages < 1 {
			t.Fatalf("empty %s PDF", tc.kind)
		}
	}
}
