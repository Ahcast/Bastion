package reports

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/xml"
	"fmt"
	"github.com/shopspring/decimal"
	"io"
	"regexp"
	"strconv"
	"time"
)

const xmlHead = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>`
const sheetNS = `http://schemas.openxmlformats.org/spreadsheetml/2006/main`
const relNS = `http://schemas.openxmlformats.org/officeDocument/2006/relationships`

var escapePattern = regexp.MustCompile(`_x[0-9A-Fa-f]{4}_`)

func escape(v string) string {
	v = escapePattern.ReplaceAllStringFunc(v, func(s string) string { return "_x005F_" + s[1:] })
	var b bytes.Buffer
	_ = xml.EscapeText(&b, []byte(v))
	return b.String()
}

type sheetWriter struct {
	w         io.Writer
	remaining *int
	err       error
}

func (w *sheetWriter) write(s string) {
	if w.err != nil {
		return
	}
	if len(s) > *w.remaining {
		w.err = exportTooLarge
		return
	}
	*w.remaining -= len(s)
	_, w.err = io.WriteString(w.w, s)
}
func (w *sheetWriter) cell(ref, value string, style int) {
	w.write(fmt.Sprintf(`<c r="%s" s="%d" t="inlineStr"><is><t xml:space="preserve">%s</t></is></c>`, ref, style, escape(value)))
}
func (w *sheetWriter) number(ref, value, formula string, style int) {
	w.write(fmt.Sprintf(`<c r="%s" s="%d">`, ref, style))
	if formula != "" {
		w.write("<f>" + escape(formula) + "</f>")
	}
	w.write("<v>" + escape(value) + "</v></c>")
}
func col(n int) string {
	out := ""
	for n > 0 {
		n--
		out = string(rune('A'+n%26)) + out
		n /= 26
	}
	return out
}

var orderHeadings = []string{"ID", "Número de orden", "Cliente", "Destino", "Fecha", "Moneda", "Estado", "Subtotal", "ISV", "Total", "Inversión de productos", "Gastos adicionales", "Inversión total", "Utilidad"}
var orderStateLabels = map[string]string{"received": "Recibida", "confirmed": "Confirmada", "purchasing": "En compra", "in_transit": "En tránsito", "goods_received": "Mercadería recibida", "preparing_delivery": "Preparando entrega", "delivered": "Entregada", "invoiced": "Facturada", "closed": "Cerrada"}

func workbook(ctx context.Context, f Filter, rows []Row, ts []Total, safety map[string]map[string]bool, generated time.Time) ([]byte, error) {
	if len(rows) > MaxExportRows {
		return nil, exportTooLarge
	}
	var b bytes.Buffer
	z := zip.NewWriter(&b)
	remaining := MaxExportXML
	part := func(name string) (*sheetWriter, error) {
		h := &zip.FileHeader{Name: name, Method: zip.Deflate}
		h.SetModTime(time.Date(1980, 1, 1, 0, 0, 0, 0, time.UTC))
		w, e := z.CreateHeader(h)
		return &sheetWriter{w: w, remaining: &remaining}, e
	}
	static := []struct{ name, data string }{
		{"[Content_Types].xml", `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>`},
		{"_rels/.rels", `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="` + relNS + `/officeDocument" Target="xl/workbook.xml"/></Relationships>`},
		{"xl/workbook.xml", `<workbook xmlns="` + sheetNS + `" xmlns:r="` + relNS + `"><sheets><sheet name="Órdenes" sheetId="1" r:id="rId1"/><sheet name="Totales" sheetId="2" r:id="rId2"/></sheets><calcPr calcId="191029" fullCalcOnLoad="1"/></workbook>`},
		{"xl/_rels/workbook.xml.rels", `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="` + relNS + `/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="` + relNS + `/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="` + relNS + `/styles" Target="styles.xml"/></Relationships>`},
		{"xl/styles.xml", `<styleSheet xmlns="` + sheetNS + `"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><color rgb="FFFFFFFF"/><name val="Calibri"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FF17365D"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="4"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFill="1" applyFont="1"/><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>`},
	}
	for _, p := range static {
		w, e := part(p.name)
		if e != nil {
			return nil, e
		}
		w.write(xmlHead + p.data)
		if w.err != nil {
			return nil, w.err
		}
	}
	p, _ := source("orders")
	moneyCols := []column{}
	for _, c := range p.columns {
		if c.money {
			moneyCols = append(moneyCols, c)
		}
	}
	w, e := part("xl/worksheets/sheet1.xml")
	if e != nil {
		return nil, e
	}
	w.write(xmlHead + `<worksheet xmlns="` + sheetNS + `"><sheetViews><sheetView workbookViewId="0"><pane ySplit="6" topLeftCell="A7" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><cols><col min="1" max="2" width="22" customWidth="1"/><col min="3" max="4" width="35" customWidth="1"/><col min="5" max="14" width="22" customWidth="1"/><col min="15" max="21" hidden="1"/></cols><sheetData>`)
	metadata := []string{"Reporte de órdenes", "Generado UTC: " + generated.UTC().Format(time.RFC3339Nano), "Desde: " + f.From + " | Hasta: " + f.To + " | Moneda: " + f.Currency + " | Estado: " + orderStateLabels[f.Status], "Importes e IDs como texto exacto. Totales SQL por moneda; fórmulas auxiliares limitadas a precisión segura."}
	for i, v := range metadata {
		n := strconv.Itoa(i + 1)
		w.write(`<row r="` + n + `">`)
		w.cell("A"+n, v, 0)
		w.write("</row>")
	}
	w.write(`<row r="6">`)
	for i, v := range orderHeadings {
		w.cell(col(i+1)+"6", v, 1)
	}
	for i, c := range moneyCols {
		w.cell(col(15+i)+"6", c.name+"_cents", 1)
	}
	w.write("</row>")
	for i, row := range rows {
		if e = ctx.Err(); e != nil {
			return nil, e
		}
		n := strconv.Itoa(i + 7)
		w.write(`<row r="` + n + `">`)
		for j, c := range p.columns {
			v := row[c.name]
			if c.name == "status" {
				v = orderStateLabels[v]
			}
			w.cell(col(j+1)+n, v, 2)
		}
		for j, c := range moneyCols {
			if safety[row["currency"]][c.name] {
				v, e := decimal.NewFromString(row[c.name])
				if e != nil {
					return nil, e
				}
				w.number(col(j+15)+n, v.Shift(2).StringFixed(0), "", 0)
			}
		}
		w.write("</row>")
		if w.err != nil {
			return nil, w.err
		}
	}
	last := len(rows) + 6
	w.write(`</sheetData><autoFilter ref="A6:N` + strconv.Itoa(last) + `"/></worksheet>`)
	if w.err != nil {
		return nil, w.err
	}
	w, e = part("xl/worksheets/sheet2.xml")
	if e != nil {
		return nil, e
	}
	w.write(xmlHead + `<worksheet xmlns="` + sheetNS + `"><cols><col min="1" max="16" width="28" customWidth="1"/></cols><sheetData><row r="1">`)
	w.cell("A1", "Totales SQL exactos. Las fórmulas auxiliares suman centavos de la misma moneda; no sustituya texto exacto por números grandes.", 0)
	w.write(`</row><row r="2">`)
	w.cell("A2", "Moneda", 1)
	w.cell("B2", "Registros", 1)
	for j := range moneyCols {
		w.cell(col(3+j*2)+"2", orderHeadings[7+j]+" exacto", 1)
		w.cell(col(4+j*2)+"2", orderHeadings[7+j]+" auxiliar", 1)
	}
	w.write("</row>")
	for i, t := range ts {
		n := strconv.Itoa(i + 3)
		w.write(`<row r="` + n + `">`)
		w.cell("A"+n, t.Currency, 2)
		w.number("B"+n, strconv.FormatInt(t.Count, 10), "", 0)
		for j, c := range moneyCols {
			w.cell(col(3+j*2)+n, t.Values[c.name], 2)
			if safety[t.Currency][c.name] && len(rows) > 0 {
				aux := col(15 + j)
				formula := fmt.Sprintf(`ROUND(SUMIF('Órdenes'!$F$7:$F$%d,"%s",'Órdenes'!$%s$7:$%s$%d)/100,2)`, last, t.Currency, aux, aux, last)
				w.number(col(4+j*2)+n, t.Values[c.name], formula, 3)
			} else {
				w.cell(col(4+j*2)+n, "Fuera de precisión Excel: use el total exacto.", 2)
			}
		}
		w.write("</row>")
	}
	w.write("</sheetData></worksheet>")
	if w.err != nil {
		return nil, w.err
	}
	if e = z.Close(); e != nil {
		return nil, e
	}
	return b.Bytes(), nil
}
