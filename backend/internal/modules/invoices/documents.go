package invoices

import (
	"bytes"
	"encoding/json"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/finance"
	"github.com/shopspring/decimal"
	"github.com/signintech/gopdf"
	"golang.org/x/image/font/gofont/goregular"
	"golang.org/x/image/font/sfnt"
	"io"
	"strconv"
	"strings"
)

type DocumentKind string

const (
	DocumentInvoice      DocumentKind = "invoice"
	DocumentDeliveryNote DocumentKind = "delivery_note"
	DocumentReceipt      DocumentKind = "receipt"
)

type DraftDocumentContext struct {
	Company        core.CompanySettings `json:"company"`
	CompanyVersion uint64               `json:"company_version,string"`
}

type PublicDocumentCompany struct {
	Name    string `json:"name"`
	TaxID   string `json:"tax_id"`
	Address string `json:"address"`
}

type PublicDocumentCustomer struct {
	Name    string `json:"name"`
	RTN     string `json:"rtn"`
	Contact string `json:"contact"`
	Phone   string `json:"phone"`
	Email   string `json:"email"`
	Address string `json:"address"`
}

type PublicDocumentFiscal struct {
	CAI           string `json:"cai"`
	Prefix        string `json:"prefix"`
	NumberWidth   uint8  `json:"number_width"`
	RangeStart    string `json:"range_start"`
	RangeEnd      string `json:"range_end"`
	ValidFrom     string `json:"valid_from"`
	ValidUntil    string `json:"valid_until"`
	Currency      string `json:"currency"`
	ConfigVersion uint64 `json:"configuration_version,string"`
}

type PublicInvoiceItem struct {
	Description     string `json:"description"`
	Unit            string `json:"unit"`
	Quantity        string `json:"quantity"`
	UnitPrice       string `json:"unit_price"`
	UnitDiscount    string `json:"unit_discount"`
	ItemDiscount    string `json:"item_discount"`
	GeneralDiscount string `json:"general_discount"`
	Base            string `json:"base"`
	Tax             string `json:"tax"`
}

type PublicDeliveryItem struct {
	Description string `json:"description"`
	Unit        string `json:"unit"`
	Quantity    string `json:"quantity"`
}

type PublicDocumentTotals struct {
	Gross           string `json:"gross"`
	ItemDiscount    string `json:"item_discount"`
	GeneralDiscount string `json:"general_discount"`
	Base15          string `json:"base_15"`
	Base18          string `json:"base_18"`
	Exempt          string `json:"exempt"`
	Exonerated      string `json:"exonerated"`
	ISV15           string `json:"isv_15"`
	ISV18           string `json:"isv_18"`
	Total           string `json:"total"`
}

type PublicInvoiceBody struct {
	Items       []PublicInvoiceItem  `json:"items"`
	Totals      PublicDocumentTotals `json:"totals"`
	Exoneration string               `json:"exoneration_reference,omitempty"`
}

type PublicDeliveryBody struct {
	Items []PublicDeliveryItem `json:"items"`
}

type PublicReceiptBody struct {
	Number    string `json:"number"`
	Date      string `json:"date"`
	Method    string `json:"method"`
	Amount    string `json:"amount"`
	Reference string `json:"reference,omitempty"`
	Voided    bool   `json:"voided"`
}

// PublicDocument is an explicit allowlist. It must never be replaced with a
// serialized Invoice or Payment because both aggregates contain private
// commercial data, ownership metadata and internal reasons.
type PublicDocument struct {
	Template       int                    `json:"template_version"`
	Kind           DocumentKind           `json:"kind"`
	Label          string                 `json:"label"`
	Company        PublicDocumentCompany  `json:"company"`
	CompanyVersion uint64                 `json:"company_version,string"`
	Customer       PublicDocumentCustomer `json:"customer"`
	InvoiceNumber  string                 `json:"invoice_number,omitempty"`
	InvoiceDate    string                 `json:"invoice_date,omitempty"`
	DueDate        string                 `json:"due_date,omitempty"`
	Currency       string                 `json:"currency,omitempty"`
	PaymentTerm    string                 `json:"payment_term_type,omitempty"`
	CreditDays     *uint16                `json:"credit_days,omitempty"`
	PublicNotes    string                 `json:"public_notes,omitempty"`
	Fiscal         *PublicDocumentFiscal  `json:"fiscal,omitempty"`
	Invoice        *PublicInvoiceBody     `json:"invoice,omitempty"`
	Delivery       *PublicDeliveryBody    `json:"delivery_note,omitempty"`
	Receipt        *PublicReceiptBody     `json:"receipt,omitempty"`
}

func publicCompany(c core.CompanySettings) PublicDocumentCompany {
	return PublicDocumentCompany{Name: c.Name, TaxID: c.TaxID, Address: c.Address}
}

func validPublicCompany(c PublicDocumentCompany) bool {
	return finance.ValidText(c.Name, 120, true) && finance.ValidText(c.TaxID, 40, true) && finance.ValidText(c.Address, 500, false)
}

func publicCustomer(c CustomerInput) PublicDocumentCustomer {
	return PublicDocumentCustomer{Name: c.Name, RTN: c.RTN, Contact: c.Contact, Phone: c.Phone, Email: c.Email, Address: c.Address}
}

func validPublicCustomer(c PublicDocumentCustomer) bool {
	return finance.ValidText(c.Name, 1000, true) && finance.ValidText(c.RTN, 30, false) && finance.ValidText(c.Contact, 120, false) && finance.ValidText(c.Phone, 50, false) && finance.ValidText(c.Email, 254, false) && finance.ValidText(c.Address, 1000, false)
}

func strictSnapshot(raw json.RawMessage, dst any) error {
	if len(raw) == 0 {
		return core.Conflict
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if e := decoder.Decode(dst); e != nil {
		return core.Conflict
	}
	var extra any
	if e := decoder.Decode(&extra); e != io.EOF {
		return core.Conflict
	}
	return nil
}

func coherentInvoiceCalculation(o Invoice) error {
	if len(o.Items) == 0 || len(o.Items) > 200 {
		return core.Conflict
	}
	calculated, e := Validate(persistedInput(o))
	if e != nil || calculated.Calculation != o.Calculation || len(calculated.Items) != len(o.Items) {
		return core.Conflict
	}
	for i := range calculated.Items {
		if calculated.Items[i] != o.Items[i].CalculatedItem {
			return core.Conflict
		}
	}
	return nil
}

func coherentFiscalNumber(number string, p FiscalAuthorization) bool {
	if !strings.HasPrefix(number, p.Prefix) {
		return false
	}
	suffix := strings.TrimPrefix(number, p.Prefix)
	if len(suffix) != int(p.Width) || suffix == "" {
		return false
	}
	for _, r := range suffix {
		if r < '0' || r > '9' {
			return false
		}
	}
	n, e := strconv.ParseUint(suffix, 10, 64)
	return e == nil && n >= p.RangeStart && n <= p.RangeEnd
}

func issuedDocumentContext(o Invoice) (PublicDocumentCompany, uint64, *PublicDocumentFiscal, error) {
	var company core.CompanySettings
	if o.Number == nil || strings.TrimSpace(*o.Number) == "" || o.IssuedAt == nil || o.IssuedBy == nil || o.DueDate == nil || o.CompanyVersion == nil || *o.CompanyVersion == 0 || len(o.Company) == 0 || len(o.SAR) == 0 {
		return PublicDocumentCompany{}, 0, nil, core.Conflict
	}
	if e := strictSnapshot(o.Company, &company); e != nil {
		return PublicDocumentCompany{}, 0, nil, e
	}
	public := publicCompany(company)
	if !validPublicCompany(public) {
		return PublicDocumentCompany{}, 0, nil, core.Conflict
	}
	var sar FiscalSnapshot
	if e := strictSnapshot(o.SAR, &sar); e != nil || sar.Version == 0 || ValidateFiscalAuthorization(sar.Configuration) != nil || !sar.Configuration.Enabled || sar.Configuration.Currency != o.Currency || o.InvoiceDate < sar.Configuration.ValidFrom || o.InvoiceDate > sar.Configuration.ValidUntil || !coherentFiscalNumber(*o.Number, sar.Configuration) {
		return PublicDocumentCompany{}, 0, nil, core.Conflict
	}
	due, e := invoiceDueDate(o)
	if e != nil || due != *o.DueDate {
		return PublicDocumentCompany{}, 0, nil, core.Conflict
	}
	fiscal := &PublicDocumentFiscal{
		CAI: sar.Configuration.CAI, Prefix: sar.Configuration.Prefix, NumberWidth: sar.Configuration.Width,
		RangeStart: fmt.Sprint(sar.Configuration.RangeStart), RangeEnd: fmt.Sprint(sar.Configuration.RangeEnd),
		ValidFrom: sar.Configuration.ValidFrom, ValidUntil: sar.Configuration.ValidUntil,
		Currency: sar.Configuration.Currency, ConfigVersion: sar.Version,
	}
	return public, *o.CompanyVersion, fiscal, nil
}

func projectInvoiceBody(o Invoice) *PublicInvoiceBody {
	items := make([]PublicInvoiceItem, 0, len(o.Items))
	for _, x := range o.Items {
		items = append(items, PublicInvoiceItem{
			Description: x.Description, Unit: x.Unit, Quantity: x.Quantity, UnitPrice: x.Price,
			UnitDiscount: x.UnitDiscount, ItemDiscount: x.ItemDiscount, GeneralDiscount: x.AllocatedDiscount,
			Base: x.Base, Tax: x.Tax,
		})
	}
	c := o.Calculation
	return &PublicInvoiceBody{Items: items, Exoneration: o.Exoneration, Totals: PublicDocumentTotals{
		Gross: c.Gross, ItemDiscount: c.ItemDiscount, GeneralDiscount: c.GeneralDiscount,
		Base15: c.Base15, Base18: c.Base18, Exempt: c.Exempt, Exonerated: c.Exonerated,
		ISV15: c.ISV15, ISV18: c.ISV18, Total: c.Total,
	}}
}

func projectDeliveryBody(o Invoice) *PublicDeliveryBody {
	items := make([]PublicDeliveryItem, 0, len(o.Items))
	for _, x := range o.Items {
		items = append(items, PublicDeliveryItem{Description: x.Description, Unit: x.Unit, Quantity: x.Quantity})
	}
	return &PublicDeliveryBody{Items: items}
}

func coherentReceiptPayment(o Invoice, p Payment) error {
	amount, e := finance.ParseDecimal(p.Amount, 2)
	if core.ID(p.ID) != nil || p.InvoiceID != o.ID || p.Version == 0 || core.ID(p.CreatedBy) != nil || p.CreatedAt.IsZero() || !fiscalDate(p.Date) || p.Date < o.InvoiceDate || !validPaymentMethod(p.Method) || p.Currency != o.Currency || e != nil || !amount.IsPositive() || !finance.ValidText(p.Reference, 500, false) {
		return core.Conflict
	}
	if p.VoidedAt == nil {
		if p.VoidedBy != nil || p.VoidReason != nil {
			return core.Conflict
		}
	} else {
		if p.VoidedBy == nil || core.ID(*p.VoidedBy) != nil || p.VoidReason == nil || !finance.ValidText(*p.VoidReason, 1000, true) {
			return core.Conflict
		}
	}
	return nil
}

// ProjectDocument performs no persistence and does not change invoice/payment
// state. Draft company data is caller-supplied and versioned; issued documents
// always use the immutable company and fiscal snapshots captured at emission.
func ProjectDocument(kind DocumentKind, o Invoice, payment *Payment, draft *DraftDocumentContext) (PublicDocument, error) {
	var out PublicDocument
	if kind != DocumentInvoice && kind != DocumentDeliveryNote && kind != DocumentReceipt {
		return out, core.Invalid
	}
	if o.ID == "" || core.ID(o.ID) != nil || !validPublicCustomer(publicCustomer(o.Customer)) || !validCurrency(o.Currency) || !fiscalDate(o.InvoiceDate) || !finance.ValidText(o.PublicNotes, 12000, false) {
		return out, core.Conflict
	}
	if e := coherentInvoiceCalculation(o); e != nil {
		return out, e
	}
	out = PublicDocument{Template: 1, Kind: kind, Customer: publicCustomer(o.Customer)}

	switch o.Status {
	case "draft":
		if kind != DocumentInvoice {
			return PublicDocument{}, core.Conflict
		}
		if draft == nil || draft.CompanyVersion == 0 {
			return PublicDocument{}, core.Invalid
		}
		company := publicCompany(draft.Company)
		if !validPublicCompany(company) {
			return PublicDocument{}, core.Invalid
		}
		if o.Number != nil || o.IssuedAt != nil || o.IssuedBy != nil || o.DueDate != nil || o.CompanyVersion != nil || len(o.Company) != 0 || len(o.SAR) != 0 {
			return PublicDocument{}, core.Conflict
		}
		due, e := invoiceDueDate(o)
		if e != nil {
			return PublicDocument{}, core.Conflict
		}
		out.Label = "Borrador - sin validez fiscal"
		out.Company = company
		out.CompanyVersion = draft.CompanyVersion
		out.InvoiceDate = o.InvoiceDate
		out.DueDate = due
		out.Currency = o.Currency
		out.PaymentTerm = o.PaymentTerm
		out.CreditDays = o.CreditDays
		out.PublicNotes = o.PublicNotes
		out.Invoice = projectInvoiceBody(o)
	case "issued":
		company, version, fiscal, e := issuedDocumentContext(o)
		if e != nil {
			return PublicDocument{}, e
		}
		out.Company = company
		out.CompanyVersion = version
		out.InvoiceNumber = *o.Number
		switch kind {
		case DocumentInvoice:
			out.Label = "Factura"
			out.InvoiceDate = o.InvoiceDate
			out.DueDate = *o.DueDate
			out.Currency = o.Currency
			out.PaymentTerm = o.PaymentTerm
			out.CreditDays = o.CreditDays
			out.PublicNotes = o.PublicNotes
			out.Fiscal = fiscal
			out.Invoice = projectInvoiceBody(o)
		case DocumentDeliveryNote:
			out.Label = "Nota de entrega - no acredita entrega física"
			out.InvoiceDate = o.InvoiceDate
			out.PublicNotes = o.PublicNotes
			out.Delivery = projectDeliveryBody(o)
		case DocumentReceipt:
			if payment == nil {
				return PublicDocument{}, core.Invalid
			}
			if e = coherentReceiptPayment(o, *payment); e != nil {
				return PublicDocument{}, e
			}
			out.Label = "Recibo"
			if payment.VoidedAt != nil {
				out.Label = "Recibo anulado"
			}
			out.Currency = o.Currency
			out.Receipt = &PublicReceiptBody{Number: "RCP-" + payment.ID, Date: payment.Date, Method: payment.Method, Amount: payment.Amount, Reference: payment.Reference, Voided: payment.VoidedAt != nil}
		}
	default:
		return PublicDocument{}, core.Conflict
	}
	if e := validatePublicDocument(out); e != nil {
		return PublicDocument{}, core.Conflict
	}
	return out, nil
}

func documentDecimal(v string, scale int, positive bool) (decimal.Decimal, error) {
	if v == "" || len(v) > 32 || strings.HasPrefix(v, ".") || strings.HasSuffix(v, ".") {
		return decimal.Zero, core.Invalid
	}
	dot := false
	fraction := 0
	for _, r := range v {
		if r == '.' {
			if dot {
				return decimal.Zero, core.Invalid
			}
			dot = true
			continue
		}
		if r < '0' || r > '9' {
			return decimal.Zero, core.Invalid
		}
		if dot {
			fraction++
		}
	}
	if fraction > scale {
		return decimal.Zero, core.Invalid
	}
	d, e := decimal.NewFromString(v)
	if e != nil || d.IsNegative() || positive && !d.IsPositive() {
		return decimal.Zero, core.Invalid
	}
	return d, nil
}

func validatePublicFiscal(f *PublicDocumentFiscal, invoiceNumber, currency, date string) error {
	if f == nil || f.ConfigVersion == 0 {
		return core.Invalid
	}
	start, e := strconv.ParseUint(f.RangeStart, 10, 64)
	if e != nil {
		return core.Invalid
	}
	end, e := strconv.ParseUint(f.RangeEnd, 10, 64)
	if e != nil {
		return core.Invalid
	}
	p := FiscalAuthorization{CAI: f.CAI, Prefix: f.Prefix, Width: f.NumberWidth, RangeStart: start, RangeEnd: end, ValidFrom: f.ValidFrom, ValidUntil: f.ValidUntil, Currency: f.Currency, Enabled: true}
	if ValidateFiscalAuthorization(p) != nil || p.Currency != currency || date < p.ValidFrom || date > p.ValidUntil || !coherentFiscalNumber(invoiceNumber, p) {
		return core.Invalid
	}
	return nil
}

func validateInvoiceBody(body *PublicInvoiceBody) error {
	if body == nil || len(body.Items) == 0 || len(body.Items) > 200 || !finance.ValidText(body.Exoneration, 500, false) {
		return core.Invalid
	}
	for _, x := range body.Items {
		if !finance.ValidText(x.Description, 4000, true) || !finance.ValidText(x.Unit, 40, false) {
			return core.Invalid
		}
		q, e := finance.ParseDecimal(x.Quantity, 4)
		if e != nil || !q.IsPositive() {
			return core.Invalid
		}
		price, e := finance.ParseDecimal(x.UnitPrice, 6)
		if e != nil {
			return core.Invalid
		}
		unitDiscount, e := documentDecimal(x.UnitDiscount, 10, false)
		if e != nil || unitDiscount.GreaterThan(price) {
			return core.Invalid
		}
		for _, money := range []string{x.ItemDiscount, x.GeneralDiscount, x.Base} {
			if _, e = finance.ParseDecimal(money, 2); e != nil {
				return core.Invalid
			}
		}
		if x.Tax != "isv15" && x.Tax != "isv18" && x.Tax != "exempt" && x.Tax != "exonerated" {
			return core.Invalid
		}
	}
	for _, money := range []string{body.Totals.Gross, body.Totals.ItemDiscount, body.Totals.GeneralDiscount, body.Totals.Base15, body.Totals.Base18, body.Totals.Exempt, body.Totals.Exonerated, body.Totals.ISV15, body.Totals.ISV18, body.Totals.Total} {
		if _, e := finance.ParseDecimal(money, 2); e != nil {
			return core.Invalid
		}
	}
	return nil
}

func validatePublicDocument(d PublicDocument) error {
	if d.Template != 1 || !validPublicCompany(d.Company) || d.CompanyVersion == 0 || !validPublicCustomer(d.Customer) {
		return core.Invalid
	}
	bodies := 0
	if d.Invoice != nil {
		bodies++
	}
	if d.Delivery != nil {
		bodies++
	}
	if d.Receipt != nil {
		bodies++
	}
	if bodies != 1 {
		return core.Invalid
	}
	switch d.Kind {
	case DocumentInvoice:
		if d.Invoice == nil || !validCurrency(d.Currency) || !fiscalDate(d.InvoiceDate) || !fiscalDate(d.DueDate) || !finance.ValidText(d.PublicNotes, 12000, false) || validateInvoiceBody(d.Invoice) != nil {
			return core.Invalid
		}
		if d.PaymentTerm == "contado" {
			if d.CreditDays != nil {
				return core.Invalid
			}
		} else if d.PaymentTerm == "credito" {
			if d.CreditDays == nil || *d.CreditDays == 0 {
				return core.Invalid
			}
		} else {
			return core.Invalid
		}
		if d.InvoiceNumber == "" {
			if d.Label != "Borrador - sin validez fiscal" || d.Fiscal != nil {
				return core.Invalid
			}
		} else {
			if d.Label != "Factura" || validatePublicFiscal(d.Fiscal, d.InvoiceNumber, d.Currency, d.InvoiceDate) != nil {
				return core.Invalid
			}
		}
	case DocumentDeliveryNote:
		if d.Delivery == nil || d.Label != "Nota de entrega - no acredita entrega física" || strings.TrimSpace(d.InvoiceNumber) == "" || !fiscalDate(d.InvoiceDate) || !finance.ValidText(d.PublicNotes, 12000, false) || d.Currency != "" || d.PaymentTerm != "" || d.CreditDays != nil || d.DueDate != "" || d.Fiscal != nil || len(d.Delivery.Items) == 0 || len(d.Delivery.Items) > 200 {
			return core.Invalid
		}
		for _, x := range d.Delivery.Items {
			q, e := finance.ParseDecimal(x.Quantity, 4)
			if e != nil || !q.IsPositive() || !finance.ValidText(x.Description, 4000, true) || !finance.ValidText(x.Unit, 40, false) {
				return core.Invalid
			}
		}
	case DocumentReceipt:
		if d.Receipt == nil || strings.TrimSpace(d.InvoiceNumber) == "" || !validCurrency(d.Currency) || d.InvoiceDate != "" || d.DueDate != "" || d.PaymentTerm != "" || d.CreditDays != nil || d.PublicNotes != "" || d.Fiscal != nil || d.Receipt.Number == "" || !fiscalDate(d.Receipt.Date) || !validPaymentMethod(d.Receipt.Method) || !finance.ValidText(d.Receipt.Reference, 500, false) {
			return core.Invalid
		}
		amount, e := finance.ParseDecimal(d.Receipt.Amount, 2)
		if e != nil || !amount.IsPositive() || d.Receipt.Voided && d.Label != "Recibo anulado" || !d.Receipt.Voided && d.Label != "Recibo" {
			return core.Invalid
		}
	default:
		return core.Invalid
	}
	return nil
}

func taxDocumentLabel(v string) string {
	switch v {
	case "isv15":
		return "ISV 15%"
	case "isv18":
		return "ISV 18%"
	case "exempt":
		return "Exento"
	case "exonerated":
		return "Exonerado"
	default:
		return ""
	}
}

// RenderDocumentPDF only accepts the public projection above. It never reads
// configuration, persistence or current payment balances and therefore cannot
// replace historical snapshots while rendering.
func RenderDocumentPDF(d PublicDocument) ([]byte, int, error) {
	if e := validatePublicDocument(d); e != nil {
		return nil, 0, e
	}
	pdf := gopdf.GoPdf{}
	pdf.Start(gopdf.Config{PageSize: *gopdf.PageSizeA4})
	if e := pdf.AddTTFFontData("Bastion", goregular.TTF); e != nil {
		return nil, 0, e
	}
	font, e := sfnt.Parse(goregular.TTF)
	if e != nil {
		return nil, 0, e
	}
	var fontBuffer sfnt.Buffer
	glyphs := func(text string) error {
		for _, r := range text {
			if r == '\n' || r == '\r' || r == '\t' {
				continue
			}
			idx, ge := font.GlyphIndex(&fontBuffer, r)
			if ge != nil || idx == 0 {
				return core.Invalid
			}
		}
		return nil
	}
	pages := 0
	y := float64(0)
	newPage := func() error {
		pages++
		if pages > 250 {
			return core.Invalid
		}
		pdf.AddPage()
		if e := pdf.SetFont("Bastion", "", 9); e != nil {
			return e
		}
		ref := d.InvoiceNumber
		if d.Kind == DocumentReceipt && d.Receipt != nil {
			ref = d.Receipt.Number + " | Factura " + d.InvoiceNumber
		}
		footer := fmt.Sprintf("Página %d", pages)
		if e := glyphs(d.Label); e != nil {
			return e
		}
		if e := glyphs(ref); e != nil {
			return e
		}
		if e := glyphs(footer); e != nil {
			return e
		}
		pdf.SetXY(42, 24)
		if e := pdf.Cell(nil, d.Label); e != nil {
			return e
		}
		if ref != "" {
			pdf.SetXY(42, 37)
			if e := pdf.Cell(nil, ref); e != nil {
				return e
			}
		}
		pdf.SetXY(42, 806)
		if e := pdf.Cell(nil, footer); e != nil {
			return e
		}
		y = 58
		return nil
	}
	if e = newPage(); e != nil {
		return nil, pages, e
	}
	line := func(text string, size float64) error { return nil }
	line = func(text string, size float64) error {
		if e := glyphs(text); e != nil {
			return e
		}
		if e := pdf.SetFont("Bastion", "", size); e != nil {
			return e
		}
		height := size + 5
		flush := func(current string) error {
			if y+height > 782 {
				if e := newPage(); e != nil {
					return e
				}
				if e := pdf.SetFont("Bastion", "", size); e != nil {
					return e
				}
			}
			pdf.SetXY(42, y)
			if e := pdf.Cell(nil, strings.TrimRight(current, " ")); e != nil {
				return e
			}
			y += height
			return nil
		}
		paragraphs := strings.Split(strings.ReplaceAll(text, "\r", ""), "\n")
		for _, paragraph := range paragraphs {
			if strings.TrimSpace(paragraph) == "" {
				if e := flush(""); e != nil {
					return e
				}
				continue
			}
			current := ""
			width := float64(0)
			for _, word := range strings.Fields(paragraph) {
				wordWidth, e := pdf.MeasureTextWidth(word + " ")
				if e != nil {
					return e
				}
				if width+wordWidth > 510 && current != "" {
					if e = flush(current); e != nil {
						return e
					}
					current = ""
					width = 0
				}
				if wordWidth > 510 {
					for _, r := range word {
						runeWidth, e := pdf.MeasureTextWidth(string(r))
						if e != nil {
							return e
						}
						if width+runeWidth > 510 && current != "" {
							if e = flush(current); e != nil {
								return e
							}
							current = ""
							width = 0
						}
						current += string(r)
						width += runeWidth
					}
					spaceWidth, e := pdf.MeasureTextWidth(" ")
					if e != nil {
						return e
					}
					current += " "
					width += spaceWidth
				} else {
					current += word + " "
					width += wordWidth
				}
			}
			if current != "" {
				if e := flush(current); e != nil {
					return e
				}
			}
		}
		return nil
	}
	write := func(text string, size float64) error {
		if strings.TrimSpace(text) == "" {
			return nil
		}
		return line(text, size)
	}
	for _, x := range []struct {
		text string
		size float64
	}{{d.Company.Name, 18}, {"RTN: " + d.Company.TaxID, 9}, {d.Company.Address, 9}, {d.Label, 16}} {
		if e = write(x.text, x.size); e != nil {
			return nil, pages, e
		}
	}
	y += 8
	if e = write("Cliente: "+d.Customer.Name, 11); e != nil {
		return nil, pages, e
	}
	for _, text := range []string{"RTN: " + d.Customer.RTN, "Contacto: " + d.Customer.Contact, d.Customer.Phone + " | " + d.Customer.Email, d.Customer.Address} {
		if e = write(text, 9); e != nil {
			return nil, pages, e
		}
	}
	y += 8

	switch d.Kind {
	case DocumentInvoice:
		if d.InvoiceNumber != "" {
			if e = write("Factura: "+d.InvoiceNumber, 11); e != nil {
				return nil, pages, e
			}
		}
		term := "Contado"
		if d.PaymentTerm == "credito" {
			term = fmt.Sprintf("Crédito (%d días)", *d.CreditDays)
		}
		for _, text := range []string{"Fecha: " + d.InvoiceDate + " | Vencimiento: " + d.DueDate, "Moneda: " + d.Currency + " | Condición: " + term} {
			if e = write(text, 10); e != nil {
				return nil, pages, e
			}
		}
		if d.Fiscal != nil {
			start, _ := strconv.ParseUint(d.Fiscal.RangeStart, 10, 64)
			end, _ := strconv.ParseUint(d.Fiscal.RangeEnd, 10, 64)
			rangeLabel := fmt.Sprintf("Rango autorizado: %s%0*d - %s%0*d", d.Fiscal.Prefix, int(d.Fiscal.NumberWidth), start, d.Fiscal.Prefix, int(d.Fiscal.NumberWidth), end)
			for _, text := range []string{"CAI: " + d.Fiscal.CAI, rangeLabel, "Vigencia: " + d.Fiscal.ValidFrom + " - " + d.Fiscal.ValidUntil} {
				if e = write(text, 9); e != nil {
					return nil, pages, e
				}
			}
		}
		y += 10
		for i, x := range d.Invoice.Items {
			for _, text := range []string{
				fmt.Sprintf("%d. %s", i+1, x.Description),
				"Cantidad: " + x.Quantity + " " + x.Unit + " | Precio unitario: " + d.Currency + " " + x.UnitPrice,
				"Descuento unitario: " + x.UnitDiscount + " | Descuento ítem: " + x.ItemDiscount + " | Descuento general: " + x.GeneralDiscount,
				"Base: " + d.Currency + " " + x.Base + " | " + taxDocumentLabel(x.Tax),
			} {
				if e = write(text, 10); e != nil {
					return nil, pages, e
				}
			}
			y += 8
		}
		t := d.Invoice.Totals
		for _, text := range []string{
			"Bruto: " + d.Currency + " " + t.Gross,
			"Descuento de ítems: " + t.ItemDiscount + " | Descuento general: " + t.GeneralDiscount,
			"Base ISV 15%: " + t.Base15 + " | Base ISV 18%: " + t.Base18,
			"Exento: " + t.Exempt + " | Exonerado: " + t.Exonerated,
			"ISV 15%: " + t.ISV15 + " | ISV 18%: " + t.ISV18,
			"TOTAL: " + d.Currency + " " + t.Total,
		} {
			if e = write(text, 11); e != nil {
				return nil, pages, e
			}
		}
		if d.Invoice.Exoneration != "" {
			if e = write("Constancia de exoneración: "+d.Invoice.Exoneration, 9); e != nil {
				return nil, pages, e
			}
		}
		if d.PublicNotes != "" {
			y += 8
			if e = write("Notas: "+d.PublicNotes, 10); e != nil {
				return nil, pages, e
			}
		}
	case DocumentDeliveryNote:
		if e = write("Factura de referencia: "+d.InvoiceNumber, 10); e != nil {
			return nil, pages, e
		}
		if e = write("Fecha de factura: "+d.InvoiceDate, 10); e != nil {
			return nil, pages, e
		}
		y += 10
		for i, x := range d.Delivery.Items {
			if e = write(fmt.Sprintf("%d. %s | Cantidad: %s %s", i+1, x.Description, x.Quantity, x.Unit), 10); e != nil {
				return nil, pages, e
			}
		}
		if d.PublicNotes != "" {
			y += 8
			if e = write("Notas: "+d.PublicNotes, 10); e != nil {
				return nil, pages, e
			}
		}
	case DocumentReceipt:
		for _, text := range []string{
			"Recibo: " + d.Receipt.Number,
			"Factura de referencia: " + d.InvoiceNumber,
			"Fecha del pago: " + d.Receipt.Date,
			"Método: " + d.Receipt.Method,
			"Referencia: " + d.Receipt.Reference,
			"Monto recibido: " + d.Currency + " " + d.Receipt.Amount,
		} {
			if e = write(text, 11); e != nil {
				return nil, pages, e
			}
		}
	}
	data, e := pdf.GetBytesPdfReturnErr()
	if e != nil {
		return nil, pages, e
	}
	if len(data) > 10*1024*1024 {
		return nil, pages, core.Invalid
	}
	return data, pages, nil
}
