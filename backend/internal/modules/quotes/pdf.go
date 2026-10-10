package quotes

import (
	"bytes"
	"fmt"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/signintech/gopdf"
	"golang.org/x/image/font/gofont/goregular"
	"golang.org/x/image/font/sfnt"
	"image/png"
	"strconv"
	"strings"
)

// This projection is the only input to rendering. Never marshal Quote into a
// public document: its aggregate contains costs, margin and private notes.
type PublicItem struct {
	Description     string `json:"description"`
	Unit            string `json:"unit"`
	Quantity        string `json:"quantity"`
	Price           string `json:"unit_price"`
	UnitDiscount    string `json:"unit_discount"`
	ItemDiscount    string `json:"item_discount"`
	GeneralDiscount string `json:"general_discount"`
	Tax             string `json:"tax"`
	Base            string `json:"base"`
}
type PublicTotals struct {
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
type PublicCustomer struct {
	Name    string `json:"name"`
	RTN     string `json:"rtn"`
	Contact string `json:"contact"`
	Phone   string `json:"phone"`
	Email   string `json:"email"`
	Address string `json:"address"`
}
type PublicQuote struct {
	Template       int                  `json:"template_version"`
	Company        core.CompanySettings `json:"company"`
	CompanyVersion uint64               `json:"company_version,string"`
	Number         string               `json:"number"`
	Version        uint64               `json:"version,string"`
	Customer       PublicCustomer       `json:"customer"`
	Reference      string               `json:"reference"`
	Date           string               `json:"date"`
	Validity       int                  `json:"validity_days"`
	Currency       string               `json:"currency"`
	Terms          string               `json:"terms"`
	Author         string               `json:"author_contact"`
	Exoneration    string               `json:"exoneration_reference"`
	Items          []PublicItem         `json:"items"`
	Totals         PublicTotals         `json:"totals"`
	Logo           []byte               `json:"logo,omitempty"`
	Stamp          []byte               `json:"stamp,omitempty"`
}

func Public(q Quote, c core.CompanySettings, cv uint64, logo, stamp []byte) PublicQuote {
	items := []PublicItem{}
	for _, x := range q.Items {
		items = append(items, PublicItem{x.Description, x.Unit, x.Quantity, x.Price, x.UnitDiscount, x.ItemDiscount, x.AllocatedDiscount, x.Tax, x.Base})
	}
	t := q.Calculation
	u := q.Customer
	return PublicQuote{1, c, cv, q.Number, q.Version, PublicCustomer{u.Name, u.RTN, u.Contact, u.Phone, u.Email, u.Address}, q.Reference, q.Date, q.ValidityDays, q.Currency, q.Terms, q.AuthorContact, q.Exoneration, items, PublicTotals{t.Gross, t.ItemDiscount, t.GeneralDiscount, t.Base15, t.Base18, t.Exempt, t.Exonerated, t.ISV15, t.ISV18, t.Total}, append([]byte{}, logo...), append([]byte{}, stamp...)}
}
func ValidateGraphic(b []byte) error {
	if len(b) == 0 {
		return nil
	}
	if len(b) > 1024*1024 {
		return core.Invalid
	}
	c, e := png.DecodeConfig(bytes.NewReader(b))
	if e != nil || c.Width < 1 || c.Height < 1 || c.Width > 2000 || c.Height > 2000 {
		return core.Invalid
	}
	_, e = png.Decode(bytes.NewReader(b))
	if e != nil {
		return core.Invalid
	}
	return nil
}

var small = []string{"cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez", "once", "doce", "trece", "catorce", "quince", "dieciséis", "diecisiete", "dieciocho", "diecinueve", "veinte", "veintiuno", "veintidós", "veintitrés", "veinticuatro", "veinticinco", "veintiséis", "veintisiete", "veintiocho", "veintinueve"}

func apocope(s string) string {
	if strings.HasSuffix(s, "veintiuno") {
		return strings.TrimSuffix(s, "veintiuno") + "veintiún"
	}
	if strings.HasSuffix(s, "uno") {
		return strings.TrimSuffix(s, "uno") + "un"
	}
	return s
}
func words(n int64) string {
	if n < 30 {
		return small[n]
	}
	if n < 100 {
		t := []string{"", "", "", "treinta", "cuarenta", "cincuenta", "sesenta", "setenta", "ochenta", "noventa"}[n/10]
		if n%10 != 0 {
			t += " y " + words(n%10)
		}
		return t
	}
	if n == 100 {
		return "cien"
	}
	if n < 1000 {
		h := []string{"", "ciento", "doscientos", "trescientos", "cuatrocientos", "quinientos", "seiscientos", "setecientos", "ochocientos", "novecientos"}[n/100]
		if n%100 != 0 {
			h += " " + words(n%100)
		}
		return h
	}
	for _, g := range []struct {
		value     int64
		one, many string
	}{{1000000000000, "un billón", "billones"}, {1000000, "un millón", "millones"}, {1000, "mil", "mil"}} {
		if n >= g.value {
			a := n / g.value
			s := g.one
			if a > 1 {
				s = apocope(words(a)) + " " + g.many
			}
			if n%g.value != 0 {
				s += " " + words(n%g.value)
			}
			return s
		}
	}
	return ""
}
func AmountWords(v, currency string) (string, error) {
	d, e := number(v, 2)
	if e != nil {
		return "", e
	}
	fixed := strings.Split(d.StringFixed(2), ".")
	n, e := strconv.ParseInt(fixed[0], 10, 64)
	if e != nil {
		return "", core.Invalid
	}
	unit := "lempiras"
	if currency == "USD" {
		unit = "dólares estadounidenses"
	} else if currency != "HNL" {
		return "", core.Invalid
	}
	if n == 1 {
		if currency == "HNL" {
			unit = "lempira"
		} else {
			unit = "dólar estadounidense"
		}
	}
	connector := " "
	if n >= 1000000 && n%1000000 == 0 {
		connector = " de "
	}
	return apocope(words(n)) + connector + unit + " con " + fixed[1] + "/100", nil
}
func RenderPDF(q PublicQuote) ([]byte, int, error) {
	return renderPDF(q, false)
}
func RenderIssuedPDF(q PublicQuote) ([]byte, int, error) {
	return renderPDF(q, true)
}
func renderPDF(q PublicQuote, issued bool) ([]byte, int, error) {
	if q.Template != 1 || ValidateGraphic(q.Logo) != nil || ValidateGraphic(q.Stamp) != nil {
		return nil, 0, core.Invalid
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
	var buffer sfnt.Buffer
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
		pdf.SetXY(42, 800)
		label := "BORRADOR - no enviada"
		if issued {
			label = "EMITIDA - no acredita envío"
		}
		if e := pdf.Cell(nil, fmt.Sprintf("%s | Versión %d | Página %d | %s", q.Number, q.Version, pages, label)); e != nil {
			return e
		}
		y = 42
		return nil
	}
	if e = newPage(); e != nil {
		return nil, pages, e
	}
	graphic := func(b []byte, x, w, h float64) error {
		if len(b) == 0 {
			return nil
		}
		holder, e := gopdf.ImageHolderByBytes(b)
		if e != nil {
			return e
		}
		cfg, e := png.DecodeConfig(bytes.NewReader(b))
		if e != nil {
			return e
		}
		ratio := float64(cfg.Width) / float64(cfg.Height)
		if w/h > ratio {
			w = h * ratio
		} else {
			h = w / ratio
		}
		return pdf.ImageByHolder(holder, x, y, &gopdf.Rect{W: w, H: h})
	}
	if e = graphic(q.Logo, 42, 95, 55); e != nil {
		return nil, pages, e
	}
	if len(q.Logo) > 0 {
		y += 65
	}
	line := func(text string, size float64) error {
		if strings.TrimSpace(text) == "" || strings.TrimSpace(text) == "|" {
			return nil
		}
		if e := pdf.SetFont("Bastion", "", size); e != nil {
			return e
		}
		current := ""
		width := float64(0)
		height := size + 5
		flush := func() error {
			if y+height > 770 {
				if e := newPage(); e != nil {
					return e
				}
				if e := pdf.SetFont("Bastion", "", size); e != nil {
					return e
				}
			}
			pdf.SetXY(42, y)
			e := pdf.Cell(nil, strings.TrimRight(current, " "))
			y += height
			current = ""
			width = 0
			return e
		}
		// Word boundary wrapping; oversized words are split by glyph, never clipped.
		for _, paragraph := range strings.Split(strings.ReplaceAll(text, "\r", ""), "\n") {
			for _, word := range strings.Fields(paragraph) {
				for _, r := range word {
					index, e := font.GlyphIndex(&buffer, r)
					if e != nil || index == 0 {
						return core.Invalid
					}
				}
				ww, e := pdf.MeasureTextWidth(word + " ")
				if e != nil {
					return e
				}
				if width+ww > 510 && current != "" {
					if e = flush(); e != nil {
						return e
					}
				}
				if ww > 510 {
					for _, r := range word {
						rw, e := pdf.MeasureTextWidth(string(r))
						if e != nil {
							return e
						}
						if width+rw > 510 {
							if e = flush(); e != nil {
								return e
							}
						}
						current += string(r)
						width += rw
					}
					current += " "
					sw, _ := pdf.MeasureTextWidth(" ")
					width += sw
				} else {
					current += word + " "
					width += ww
				}
			}
			if e := flush(); e != nil {
				return e
			}
		}
		return nil
	}
	write := func(text string, size float64) error { return line(text, size) }
	lines := []struct {
		s    string
		size float64
	}{{q.Company.Name, 18}, {"RTN: " + q.Company.TaxID + " | " + q.Company.Address, 9}, {"COTIZACIÓN " + q.Number, 16}, {fmt.Sprintf("Fecha: %s | Validez: %d días | Moneda: %s", q.Date, q.Validity, q.Currency), 10}, {"Cliente: " + q.Customer.Name, 11}, {"RTN: " + q.Customer.RTN + " | Contacto: " + q.Customer.Contact, 9}, {q.Customer.Phone + " | " + q.Customer.Email, 9}, {q.Customer.Address, 9}, {"Expediente / referencia: " + q.Reference, 10}}
	for _, x := range lines {
		if e = write(x.s, x.size); e != nil {
			return nil, pages, e
		}
	}
	y += 12
	taxLabel := map[string]string{"isv15": "ISV 15%", "isv18": "ISV 18%", "exempt": "Exento", "exonerated": "Exonerado"}
	for i, x := range q.Items {
		if y+70 > 770 {
			if e = newPage(); e != nil {
				return nil, pages, e
			}
		}
		for _, l := range []string{fmt.Sprintf("%d. %s", i+1, x.Description), "Cantidad: " + x.Quantity + " " + x.Unit + " | Precio unitario: " + q.Currency + " " + x.Price, "Descuento por unidad: " + x.UnitDiscount + " | Descuento ítem: " + x.ItemDiscount + " | General asignado: " + x.GeneralDiscount, "Base neta: " + q.Currency + " " + x.Base + " | " + taxLabel[x.Tax]} {
			if e = write(l, 10); e != nil {
				return nil, pages, e
			}
		}
		y += 10
	}
	t := q.Totals
	y += 10
	amount, e := AmountWords(t.Total, q.Currency)
	if e != nil {
		return nil, pages, e
	}
	for _, l := range []string{"Bruto: " + t.Gross, "Descuento de ítems: " + t.ItemDiscount + " | Descuento general: " + t.GeneralDiscount, "Base ISV 15%: " + t.Base15 + " | Base ISV 18%: " + t.Base18, "Exento: " + t.Exempt + " | Exonerado: " + t.Exonerated, "ISV 15%: " + t.ISV15 + " | ISV 18%: " + t.ISV18, "TOTAL: " + q.Currency + " " + t.Total, amount} {
		if e = write(l, 11); e != nil {
			return nil, pages, e
		}
	}
	if q.Exoneration != "" {
		if e = write("Constancia de exoneración: "+q.Exoneration, 10); e != nil {
			return nil, pages, e
		}
	}
	y += 12
	for _, l := range []string{"Términos y condiciones", q.Terms, "Contacto del autor: " + q.Author} {
		if e = write(l, 10); e != nil {
			return nil, pages, e
		}
	}
	if len(q.Stamp) > 0 {
		if y+65 > 770 {
			if e = newPage(); e != nil {
				return nil, pages, e
			}
		}
		if e = graphic(q.Stamp, 42, 100, 55); e != nil {
			return nil, pages, e
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
