package integration

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"strings"
	"sync"
	"testing"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/invoices"
)

type invoiceDocumentStore struct {
	mu       sync.Mutex
	files    map[string][]byte
	failNext int
	writes   int
}

func newInvoiceDocumentStore() *invoiceDocumentStore {
	return &invoiceDocumentStore{files: map[string][]byte{}}
}

func (s *invoiceDocumentStore) PutImmutable(_ context.Context, key string, data []byte, checksum string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.failNext > 0 {
		s.failNext--
		return errors.New("synthetic immutable storage outage")
	}
	if security.Hash(string(data)) != checksum {
		return errors.New("checksum mismatch")
	}
	if previous, ok := s.files[key]; ok {
		if !bytes.Equal(previous, data) {
			return errors.New("immutable object changed")
		}
		return nil
	}
	s.files[key] = append([]byte(nil), data...)
	s.writes++
	return nil
}

func (s *invoiceDocumentStore) stats() (int, int) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return len(s.files), s.writes
}

func configureInvoiceDocuments(t *testing.T, s *invoices.Service, store *invoiceDocumentStore) {
	t.Helper()
	s.Core.DocumentPolicies["invoices"] = s.DocumentPolicy
	invoices.ConfigureDocumentStore(s, store)
	t.Cleanup(func() { invoices.ConfigureDocumentStore(s, nil) })
}

func generatedDocumentResult(t *testing.T, value any, e error) map[string]string {
	t.Helper()
	if e != nil {
		t.Fatal(e)
	}
	raw, e := json.Marshal(value)
	if e != nil {
		t.Fatal(e)
	}
	var out map[string]string
	if e = json.Unmarshal(raw, &out); e != nil {
		t.Fatal(e)
	}
	return out
}

func mustGenerateDocument(t *testing.T, s *invoices.Service, a core.Actor, id, key string, input invoices.GenerateDocumentInput) map[string]string {
	t.Helper()
	value, e := s.GenerateDocument(ctx, a, id, key, input)
	return generatedDocumentResult(t, value, e)
}

func assertPublicSnapshotPrivate(t *testing.T, raw string, forbidden ...string) {
	t.Helper()
	for _, fragment := range forbidden {
		if strings.Contains(raw, fragment) {
			t.Fatalf("public snapshot leaked %q: %s", fragment, raw)
		}
	}
}

func TestInvoiceGeneratedDraftDocumentPersistsSafeSnapshotAndReplays(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	store := newInvoiceDocumentStore()
	configureInvoiceDocuments(t, s, store)
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Castillo IT actual","tax_id":"08011900123456","address":"Tegucigalpa"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	value, e := s.Save(ctx, a, "", "document-draft-create", invoiceInput())
	id := customerResult(t, value, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	input := invoices.GenerateDocumentInput{Kind: invoices.DocumentInvoice, Version: o.Version}
	first := mustGenerateDocument(t, s, a, id, "document-draft-pdf", input)
	if first["status"] != "available" || first["document_id"] == "" || first["kind"] != string(invoices.DocumentInvoice) || first["invoice_version"] != "1" {
		t.Fatal(first)
	}
	var publicData, objectKey, status string
	var documentID string
	if e = s.Core.DB.QueryRow(`SELECT public_data,object_key,status,CAST(document_id AS CHAR) FROM invoice_document_snapshots WHERE id=?`, first["id"]).Scan(&publicData, &objectKey, &status, &documentID); e != nil {
		t.Fatal(e)
	}
	if status != "available" || documentID != first["document_id"] || !strings.Contains(publicData, `"label":"Borrador - sin validez fiscal"`) || !strings.Contains(publicData, `"name":"Castillo IT actual"`) || strings.TrimSpace(objectKey) == "" {
		t.Fatal(status, documentID, objectKey, publicData)
	}
	assertPublicSnapshotPrivate(t, publicData,
		`"unit_cost"`, `"cost"`, `"products_investment"`, `"additional_expenses"`, `"investment_total"`, `"profit"`,
		`"created_by"`, `"team_id"`, `"paid_total"`, `"balance"`, `"source_quote_id"`, `"source_purchase_order_id"`)
	var entityType, entityID, mediaType string
	var size int64
	if e = s.Core.DB.QueryRow(`SELECT d.entity_type,CAST(d.entity_id AS CHAR),v.media_type,v.size FROM documents d JOIN document_versions v ON v.document_id=d.id AND v.version=1 WHERE d.id=?`, first["document_id"]).Scan(&entityType, &entityID, &mediaType, &size); e != nil {
		t.Fatal(e)
	}
	if entityType != "invoices" || entityID != id || mediaType != "application/pdf" || size <= 0 {
		t.Fatal(entityType, entityID, mediaType, size)
	}
	replay := mustGenerateDocument(t, s, a, id, "document-draft-pdf", input)
	if replay["id"] != first["id"] || replay["document_id"] != first["document_id"] || replay["status"] != "available" {
		t.Fatal(replay, first)
	}
	objects, writes := store.stats()
	if objects != 1 || writes != 1 {
		t.Fatal(objects, writes)
	}
	listed, e := s.GeneratedDocuments(ctx, a, id, 1, 25)
	if e != nil {
		t.Fatal(e)
	}
	raw, e := json.Marshal(listed)
	if e != nil {
		t.Fatal(e)
	}
	text := string(raw)
	if !strings.Contains(text, first["document_id"]) || strings.Contains(text, "public_data") || strings.Contains(text, "object_key") || strings.Contains(text, "input_hash") || strings.Contains(text, "unit_cost") {
		t.Fatal(text)
	}
}

func TestInvoiceGeneratedDocumentStorageFailureIsResumable(t *testing.T) {
	s, a := invoiceFixture(t)
	grantInvoice(t, s.Core, a, "all")
	store := newInvoiceDocumentStore()
	store.failNext = 1
	configureInvoiceDocuments(t, s, store)
	if e := s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 0, Value: []byte(`{"name":"Empresa reintento","tax_id":"08011900123456","address":"Tegucigalpa"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	value, e := s.Save(ctx, a, "", "document-retry-create", invoiceInput())
	id := customerResult(t, value, e)["id"]
	o, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	input := invoices.GenerateDocumentInput{Kind: invoices.DocumentInvoice, Version: o.Version}
	if _, e = s.GenerateDocument(ctx, a, id, "document-retry-key", input); e == nil || !strings.Contains(e.Error(), "synthetic immutable storage outage") {
		t.Fatal("expected storage failure", e)
	}
	var snapshotID, status string
	var documentID *string
	if e = s.Core.DB.QueryRow(`SELECT CAST(id AS CHAR),status,CAST(document_id AS CHAR) FROM invoice_document_snapshots WHERE invoice_id=?`, id).Scan(&snapshotID, &status, &documentID); e != nil {
		t.Fatal(e)
	}
	if status != "pending" || documentID != nil {
		t.Fatal(snapshotID, status, documentID)
	}
	result := mustGenerateDocument(t, s, a, id, "document-retry-key", input)
	if result["id"] != snapshotID || result["status"] != "available" || result["document_id"] == "" {
		t.Fatal(result, snapshotID)
	}
	var snapshots, documents int
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM invoice_document_snapshots WHERE invoice_id=?`, id).Scan(&snapshots); e != nil {
		t.Fatal(e)
	}
	if e = s.Core.DB.QueryRow(`SELECT COUNT(*) FROM documents WHERE entity_type='invoices' AND entity_id=?`, id).Scan(&documents); e != nil {
		t.Fatal(e)
	}
	objects, writes := store.stats()
	if snapshots != 1 || documents != 1 || objects != 1 || writes != 1 {
		t.Fatal(snapshots, documents, objects, writes)
	}
}

func TestInvoiceGeneratedIssuedDeliveryAndReceiptUseHistoricalEvidence(t *testing.T) {
	s, a, id, emission := invoiceEmissionFixture(t)
	store := newInvoiceDocumentStore()
	configureInvoiceDocuments(t, s, store)
	if _, e := s.Emit(ctx, a, id, "document-issued-emit", emission); e != nil {
		t.Fatal(e)
	}
	issued, e := s.Get(ctx, a, id)
	if e != nil || issued.Status != "issued" || issued.Number == nil {
		t.Fatal(issued, e)
	}
	if e = s.Core.SaveSetting(ctx, a, "company", core.Setting{Version: 1, Value: []byte(`{"name":"Empresa posterior no histórica","tax_id":"08011900999999","address":"Otra dirección"}`)}, ""); e != nil {
		t.Fatal(e)
	}
	invoicePDF := mustGenerateDocument(t, s, a, id, "document-issued-invoice", invoices.GenerateDocumentInput{Kind: invoices.DocumentInvoice, Version: issued.Version})
	var invoicePublic string
	if e = s.Core.DB.QueryRow(`SELECT public_data FROM invoice_document_snapshots WHERE id=?`, invoicePDF["id"]).Scan(&invoicePublic); e != nil {
		t.Fatal(e)
	}
	if !strings.Contains(invoicePublic, "Empresa fiscal sintética") || strings.Contains(invoicePublic, "Empresa posterior no histórica") || !strings.Contains(invoicePublic, "CAI-SINTETICO-NO-VALIDO") {
		t.Fatal(invoicePublic)
	}
	assertPublicSnapshotPrivate(t, invoicePublic, `"unit_cost"`, `"profit"`, `"paid_total"`, `"balance"`)

	delivery := mustGenerateDocument(t, s, a, id, "document-issued-delivery", invoices.GenerateDocumentInput{Kind: invoices.DocumentDeliveryNote, Version: issued.Version})
	var deliveryPublic string
	if e = s.Core.DB.QueryRow(`SELECT public_data FROM invoice_document_snapshots WHERE id=?`, delivery["id"]).Scan(&deliveryPublic); e != nil {
		t.Fatal(e)
	}
	if !strings.Contains(deliveryPublic, "Nota de entrega - no acredita entrega física") {
		t.Fatal(deliveryPublic)
	}
	assertPublicSnapshotPrivate(t, deliveryPublic, `"currency"`, `"unit_price"`, `"tax"`, `"totals"`, `"fiscal"`, `"cai"`, `"paid_total"`, `"balance"`)

	paymentInput := invoices.PaymentInput{Version: issued.Version, Date: invoiceToday(), Method: "Transferencia", Currency: issued.Currency, Amount: "50.00", Reference: "DOC-RECEIPT-1", Reason: "razón privada que no debe publicarse"}
	paymentValue, paymentError := s.RegisterPayment(ctx, a, id, "document-receipt-payment", paymentInput)
	payment := paymentResult(t, paymentValue, paymentError)
	afterPayment, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	receipt := mustGenerateDocument(t, s, a, id, "document-receipt-active", invoices.GenerateDocumentInput{Kind: invoices.DocumentReceipt, Version: afterPayment.Version, PaymentID: &payment.PaymentID, PaymentVersion: &payment.PaymentVersion})
	var receiptPublic string
	if e = s.Core.DB.QueryRow(`SELECT public_data FROM invoice_document_snapshots WHERE id=?`, receipt["id"]).Scan(&receiptPublic); e != nil {
		t.Fatal(e)
	}
	if !strings.Contains(receiptPublic, `"label":"Recibo"`) || !strings.Contains(receiptPublic, `"number":"RCP-`+payment.PaymentID+`"`) || !strings.Contains(receiptPublic, `"amount":"50.00"`) {
		t.Fatal(receiptPublic)
	}
	assertPublicSnapshotPrivate(t, receiptPublic, "razón privada", `"reason"`, `"void_reason"`, `"paid_total"`, `"balance"`)

	voidValue, voidError := s.VoidPayment(ctx, a, id, payment.PaymentID, "document-receipt-void", invoices.VoidPaymentInput{Version: afterPayment.Version, PaymentVersion: payment.PaymentVersion, Reason: "motivo interno de anulación"})
	voided := paymentResult(t, voidValue, voidError)
	afterVoid, e := s.Get(ctx, a, id)
	if e != nil {
		t.Fatal(e)
	}
	voidReceipt := mustGenerateDocument(t, s, a, id, "document-receipt-voided", invoices.GenerateDocumentInput{Kind: invoices.DocumentReceipt, Version: afterVoid.Version, PaymentID: &payment.PaymentID, PaymentVersion: &voided.PaymentVersion})
	var voidPublic string
	if e = s.Core.DB.QueryRow(`SELECT public_data FROM invoice_document_snapshots WHERE id=?`, voidReceipt["id"]).Scan(&voidPublic); e != nil {
		t.Fatal(e)
	}
	if !strings.Contains(voidPublic, `"label":"Recibo anulado"`) || !strings.Contains(voidPublic, `"voided":true`) {
		t.Fatal(voidPublic)
	}
	assertPublicSnapshotPrivate(t, voidPublic, "motivo interno", "razón privada", `"reason"`, `"void_reason"`, `"paid_total"`, `"balance"`)
	objects, writes := store.stats()
	if objects != 4 || writes != 4 {
		t.Fatal(objects, writes)
	}
}
