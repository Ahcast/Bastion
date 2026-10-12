package reports

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/xml"
	"io"
	"os"
	"strings"
	"testing"
	"time"
)

type xmlCell struct {
	Ref     string `xml:"r,attr"`
	Type    string `xml:"t,attr"`
	Formula string `xml:"f"`
	Value   string `xml:"v"`
	Text    string `xml:"is>t"`
}
type xmlSheet struct {
	Rows []struct {
		Cells []xmlCell `xml:"c"`
	} `xml:"sheetData>row"`
}

func cells(t *testing.T, data []byte, name string) map[string]xmlCell {
	t.Helper()
	z, e := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if e != nil {
		t.Fatal(e)
	}
	out := map[string]xmlCell{}
	for _, f := range z.File {
		if f.Name == name {
			r, e := f.Open()
			if e != nil {
				t.Fatal(e)
			}
			defer r.Close()
			var s xmlSheet
			if e = xml.NewDecoder(r).Decode(&s); e != nil {
				t.Fatal(e)
			}
			for _, row := range s.Rows {
				for _, c := range row.Cells {
					out[c.Ref] = c
				}
			}
			return out
		}
	}
	t.Fatal("missing part", name)
	return nil
}
func sampleWorkbook(t *testing.T) []byte {
	t.Helper()
	p, _ := source("orders")
	row := Row{"id": "9007199254740993", "order_number": "=HYPERLINK(\"https://example.test\")", "customer": "+Cliente áé & <texto>", "destination": "@Destino", "order_date": "2026-10-12", "currency": "HNL", "status": "received"}
	total := Total{Currency: "HNL", Count: 1, Values: Row{}}
	safety := map[string]map[string]bool{"HNL": {}}
	for _, c := range p.columns {
		if c.money {
			row[c.name] = "135.01"
			total.Values[c.name] = row[c.name]
			safety["HNL"][c.name] = true
		}
	}
	row["profit"] = "-20.01"
	total.Values["profit"] = "-20.01"
	large := Row{}
	for k, v := range row {
		large[k] = v
	}
	large["id"] = "9007199254740995"
	large["currency"] = "USD"
	big := Total{Currency: "USD", Count: 1, Values: Row{}}
	for _, c := range p.columns {
		if c.money {
			large[c.name] = "9200000000000000.01"
			big.Values[c.name] = large[c.name]
		}
	}
	rows := []Row{row, large}
	ts := []Total{total, big}
	when := time.Date(2026, 10, 12, 20, 0, 0, 0, time.UTC)
	data, e := workbook(context.Background(), Filter{From: "2026-10-01", To: "2026-10-12"}, rows, ts, safety, when)
	if e != nil {
		t.Fatal(e)
	}
	again, e := workbook(context.Background(), Filter{From: "2026-10-01", To: "2026-10-12"}, rows, ts, safety, when)
	if e != nil || !bytes.Equal(data, again) {
		t.Fatal("non deterministic", e)
	}
	return data
}
func TestWorkbookExactStringsControlledFormulasAndPackaging(t *testing.T) {
	data := sampleWorkbook(t)
	c := cells(t, data, "xl/worksheets/sheet1.xml")
	for ref, v := range map[string]string{"A7": "9007199254740993", "B7": "=HYPERLINK(\"https://example.test\")", "C7": "+Cliente áé & <texto>", "D7": "@Destino", "G7": "Recibida", "N7": "-20.01", "J8": "9200000000000000.01"} {
		if c[ref].Text != v || c[ref].Type != "inlineStr" || c[ref].Formula != "" {
			t.Fatal(ref, c[ref])
		}
	}
	if c["U7"].Value != "-2001" {
		t.Fatal(c["U7"])
	}
	totals := cells(t, data, "xl/worksheets/sheet2.xml")
	if totals["P3"].Formula != `ROUND(SUMIF('Órdenes'!$F$7:$F$8,"HNL",'Órdenes'!$U$7:$U$8)/100,2)` || totals["P3"].Value != "-20.01" {
		t.Fatal(totals["P3"])
	}
	if totals["G4"].Text != "9200000000000000.01" || totals["H4"].Formula != "" || !strings.Contains(totals["H4"].Text, "precisión") {
		t.Fatal(totals["H4"])
	}
	z, e := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if e != nil {
		t.Fatal(e)
	}
	for _, f := range z.File {
		if strings.Contains(f.Name, "vba") || strings.Contains(f.Name, "external") {
			t.Fatal(f.Name)
		}
		r, e := f.Open()
		if e != nil {
			t.Fatal(e)
		}
		raw, e := io.ReadAll(r)
		r.Close()
		if e != nil || strings.Contains(string(raw), `TargetMode="External"`) {
			t.Fatal(f.Name, e)
		}
		decoder := xml.NewDecoder(bytes.NewReader(raw))
		for {
			_, e = decoder.Token()
			if e == io.EOF {
				break
			}
			if e != nil {
				t.Fatal(f.Name, e)
			}
		}
	}
	if path := os.Getenv("BASTION_TEST_REPORT_XLSX"); path != "" {
		if e = os.WriteFile(path, data, 0600); e != nil {
			t.Fatal(e)
		}
	}
}
func TestWorkbookRejectsOversizeAndCancellation(t *testing.T) {
	rows := make([]Row, MaxExportRows+1)
	if _, e := workbook(context.Background(), Filter{}, rows, nil, nil, time.Now()); e != exportTooLarge {
		t.Fatal(e)
	}
	rows = make([]Row, MaxExportRows)
	for i := range rows {
		rows[i] = Row{"id": "1", "customer": strings.Repeat("á", 1000), "destination": strings.Repeat("é", 1000)}
	}
	if _, e := workbook(context.Background(), Filter{}, rows, nil, nil, time.Now()); e != exportTooLarge {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, e := workbook(ctx, Filter{}, []Row{{}}, nil, nil, time.Now()); e != context.Canceled {
		t.Fatal(e)
	}
}
