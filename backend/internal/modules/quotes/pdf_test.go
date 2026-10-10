package quotes

import (
	"bytes"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"os"
	"strings"
	"testing"
)

func publicFixture() PublicQuote {
	p := CalculationInput{Items: []Item{item("2", "100", "10", "isv15")}, DiscountType: "amount", Discount: "5"}
	r, _ := Calculate(p)
	q := Quote{ID: "private-id", Number: "COT-000001", Version: 7, Customer: CustomerInput{ID: "private-client-id", Name: "Institución sintética", RTN: "08011990123456", Contact: "Contacto Ejemplo", Address: "Dirección de prueba"}, Source: "private-process-id", Reference: "EXP-SINTÉTICO-1", Date: "2026-10-10", ValidityDays: 30, Currency: "HNL", Terms: "Entrega según términos acordados. Esta es una prueba sintética.", Notes: "PRIVATE-SECRET", AuthorContact: "Autor Ejemplo | ejemplo@example.test", Result: r}
	return Public(q, core.CompanySettings{Name: "Empresa sintética", TaxID: "08011900123456", Address: "Dirección sintética"}, 1, nil, nil)
}
func TestPublicPDFProjectionPrivacyAndDeterministicRendering(t *testing.T) {
	p := publicFixture()
	b, _ := json.Marshal(p)
	for _, x := range []string{"PRIVATE-SECRET", "private-id", "private-client-id", "private-process-id", "unit_cost", "margin", "created_by", "source_process_id"} {
		if bytes.Contains(b, []byte(x)) {
			t.Fatal("public leak", x)
		}
	}
	data, pages, e := RenderPDF(p)
	if e != nil || pages != 1 || !bytes.HasPrefix(data, []byte("%PDF-")) {
		t.Fatal(pages, e)
	}
	for i := 0; i < 5; i++ {
		next, _, e := RenderPDF(p)
		if e != nil || !bytes.Equal(data, next) {
			t.Fatal("rendering not deterministic", e)
		}
	}
	if path := os.Getenv("BASTION_PDF_TEST_OUTPUT"); path != "" {
		if e = os.WriteFile(path, data, 0600); e != nil {
			t.Fatal(e)
		}
	}
}
func TestPDFWrapLongWordsPaginationAndUnsupportedGlyph(t *testing.T) {
	p := publicFixture()
	p.Items[0].Description = strings.Repeat("Especificación sintética con acentos y caracteres. ", 200) + strings.Repeat("A", 4000)
	p.Terms = strings.Repeat("Condición pública de entrega. ", 500)
	data, pages, e := RenderPDF(p)
	if e != nil || pages < 3 || len(data) < 1000 {
		t.Fatal(pages, e)
	}
	p.Items[0].Description = "Sin soporte 🔥"
	if _, _, e = RenderPDF(p); e != core.Invalid {
		t.Fatal("glyph omitted", e)
	}
	if path := os.Getenv("BASTION_PDF_LONG_OUTPUT"); path != "" {
		p.Items[0].Description = strings.Repeat("Especificación sintética. ", 500)
		data, _, e = RenderPDF(p)
		if e != nil {
			t.Fatal(e)
		}
		if e = os.WriteFile(path, data, 0600); e != nil {
			t.Fatal(e)
		}
	}
}
func TestTotalInSpanishWordsAndGraphicValidation(t *testing.T) {
	for _, x := range []struct{ v, c, w string }{{"0.00", "HNL", "cero lempiras con 00/100"}, {"1.05", "USD", "un dólar estadounidense con 05/100"}, {"21.30", "HNL", "veintiún lempiras con 30/100"}, {"1000", "HNL", "mil lempiras con 00/100"}, {"1000000", "HNL", "un millón de lempiras con 00/100"}, {"2000000000000", "HNL", "dos billones de lempiras con 00/100"}} {
		s, e := AmountWords(x.v, x.c)
		if e != nil || s != x.w {
			t.Fatal(x, s, e)
		}
	}
	if ValidateGraphic([]byte("notPNG")) != core.Invalid {
		t.Fatal("invalid graphic")
	}
}
