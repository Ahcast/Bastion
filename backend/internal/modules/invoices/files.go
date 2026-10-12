package invoices

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
)

type PDFStore interface {
	PutImmutable(context.Context, string, []byte, string) error
}

// Stores are wired once by composition. Keeping the mapping keyed by Service
// preserves test isolation without making business code depend on the S3 adapter.
var documentStores sync.Map

func ConfigureDocumentStore(service *Service, store PDFStore) {
	if service == nil {
		return
	}
	if store == nil {
		documentStores.Delete(service)
		return
	}
	documentStores.Store(service, store)
}

func (s *Service) documentStore() PDFStore {
	value, ok := documentStores.Load(s)
	if !ok {
		return nil
	}
	store, _ := value.(PDFStore)
	return store
}

type GenerateDocumentInput struct {
	Kind           DocumentKind `json:"kind"`
	Version        uint64       `json:"version,string"`
	PaymentID      *string      `json:"payment_id"`
	PaymentVersion *uint64      `json:"payment_version,string"`
}

type documentSnapshot struct {
	ID             string
	DocumentID     string
	Kind           DocumentKind
	Status         string
	PublicData     string
	ObjectKey      string
	InputHash      string
	InvoiceVersion uint64
	PaymentID      *string
	PaymentVersion *uint64
}

type GeneratedDocument struct {
	ID             string     `json:"id"`
	DocumentID     *string    `json:"document_id"`
	Kind           string     `json:"kind"`
	Status         string     `json:"status"`
	InvoiceVersion string     `json:"invoice_version"`
	PaymentID      *string    `json:"payment_id"`
	PaymentVersion *string    `json:"payment_version"`
	CreatedAt      time.Time  `json:"created_at"`
	AvailableAt    *time.Time `json:"available_at"`
}

func (s *Service) DocumentPolicy(ctx context.Context, q core.Querier, a core.Actor, id string, write bool) error {
	permission := "facturas.view"
	if write {
		permission = "facturas.documents.generate"
	}
	_, e := s.access(ctx, q, a, id, permission, write)
	return e
}

func validGenerateDocumentInput(id, key string, p GenerateDocumentInput) error {
	if core.ID(id) != nil || key == "" || p.Version == 0 {
		return core.Invalid
	}
	switch p.Kind {
	case DocumentInvoice, DocumentDeliveryNote:
		if p.PaymentID != nil || p.PaymentVersion != nil {
			return core.Invalid
		}
	case DocumentReceipt:
		if p.PaymentID == nil || p.PaymentVersion == nil || core.ID(*p.PaymentID) != nil || *p.PaymentVersion == 0 {
			return core.Invalid
		}
	default:
		return core.Invalid
	}
	return nil
}

func loadDocumentPayment(ctx context.Context, q core.Querier, invoiceID, paymentID string, lock bool) (Payment, error) {
	var p Payment
	if core.ID(paymentID) != nil {
		return p, core.Invalid
	}
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT `+paymentColumns+` FROM payments WHERE invoice_id=? AND id=?`+suffix, invoiceID, paymentID).Scan(paymentTargets(&p)...)
	if e == sql.ErrNoRows {
		return Payment{}, core.Missing
	}
	return p, e
}

func (s *Service) projectStoredDocument(ctx context.Context, tx *sql.Tx, o *Invoice, p GenerateDocumentInput) (PublicDocument, error) {
	if e := s.items(ctx, tx, o); e != nil {
		return PublicDocument{}, e
	}
	var payment *Payment
	if p.Kind == DocumentReceipt {
		loaded, e := loadDocumentPayment(ctx, tx, o.ID, *p.PaymentID, true)
		if e != nil {
			return PublicDocument{}, e
		}
		if loaded.Version != *p.PaymentVersion {
			return PublicDocument{}, core.Conflict
		}
		payment = &loaded
	}
	var draft *DraftDocumentContext
	if o.Status == "draft" {
		company, version, e := s.Core.PublicCompany(ctx, tx)
		if e != nil {
			return PublicDocument{}, e
		}
		draft = &DraftDocumentContext{Company: company, CompanyVersion: version}
	}
	return ProjectDocument(p.Kind, *o, payment, draft)
}

func documentInputHash(raw []byte) string {
	return security.Hash("invoice-document:v1:" + string(raw))
}

func loadDocumentSnapshot(ctx context.Context, q core.Querier, invoiceID, snapshotID string, lock bool) (documentSnapshot, error) {
	var out documentSnapshot
	var documentID, paymentID, paymentVersion sql.NullString
	suffix := ""
	if lock {
		suffix = " FOR UPDATE"
	}
	e := q.QueryRowContext(ctx, `SELECT CAST(id AS CHAR),CAST(document_id AS CHAR),kind,status,public_data,object_key,input_hash,invoice_version,CAST(payment_id AS CHAR),CAST(payment_version AS CHAR) FROM invoice_document_snapshots WHERE id=? AND invoice_id=?`+suffix, snapshotID, invoiceID).Scan(&out.ID, &documentID, &out.Kind, &out.Status, &out.PublicData, &out.ObjectKey, &out.InputHash, &out.InvoiceVersion, &paymentID, &paymentVersion)
	if e == sql.ErrNoRows {
		return out, core.Missing
	}
	if e != nil {
		return out, e
	}
	out.DocumentID = documentID.String
	if paymentID.Valid {
		v := paymentID.String
		out.PaymentID = &v
	}
	if paymentVersion.Valid {
		var v uint64
		if _, e = fmt.Sscan(paymentVersion.String, &v); e != nil || v == 0 {
			return documentSnapshot{}, core.Conflict
		}
		out.PaymentVersion = &v
	}
	if out.Status != "pending" && out.Status != "available" {
		return documentSnapshot{}, core.Conflict
	}
	return out, nil
}

func (v documentSnapshot) result() map[string]string {
	return map[string]string{
		"id":              v.ID,
		"document_id":     v.DocumentID,
		"kind":            string(v.Kind),
		"invoice_version": fmt.Sprint(v.InvoiceVersion),
		"status":          v.Status,
	}
}

func (s *Service) GenerateDocument(ctx context.Context, a core.Actor, id, key string, p GenerateDocumentInput) (any, error) {
	if a.Type != "user" {
		return nil, core.Forbidden
	}
	if e := validGenerateDocumentInput(id, key, p); e != nil {
		return nil, e
	}
	store := s.documentStore()
	if store == nil || !s.Core.IntegrationEnabled(ctx, "files") {
		return nil, core.Disabled
	}

	var snap documentSnapshot
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "facturas.documents.generate"); e != nil {
			return e
		}
		if e := s.Core.Authorize(ctx, tx, a, "core.documents.write", a.UserID, "", false); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "facturas.documents.generate", true)
		if e != nil {
			return e
		}
		reserved, e := s.Core.Idempotent(ctx, tx, a, "invoices.document:"+id, key, p, func() (any, error) {
			if o.Version != p.Version {
				return nil, core.Conflict
			}
			public, e := s.projectStoredDocument(ctx, tx, &o, p)
			if e != nil {
				return nil, e
			}
			raw, e := json.Marshal(public)
			if e != nil {
				return nil, e
			}
			if len(raw) > 3*1024*1024 {
				return nil, core.Invalid
			}
			hash := documentInputHash(raw)
			var snapshotID string
			e = tx.QueryRowContext(ctx, `SELECT CAST(id AS CHAR) FROM invoice_document_snapshots WHERE invoice_id=? AND kind=? AND input_hash=?`, id, p.Kind, hash).Scan(&snapshotID)
			if e == sql.ErrNoRows {
				var paymentID, paymentVersion any
				if p.PaymentID != nil {
					paymentID = *p.PaymentID
					paymentVersion = *p.PaymentVersion
				}
				r, e := tx.ExecContext(ctx, `INSERT INTO invoice_document_snapshots(invoice_id,invoice_version,payment_id,payment_version,kind,input_hash,public_data,object_key,status,created_by,created_at) VALUES(?,?,?,?,?,?,?,?,'pending',?,UTC_TIMESTAMP(6))`, id, p.Version, paymentID, paymentVersion, p.Kind, hash, string(raw), "invoices/"+id+"/documents/"+string(p.Kind)+"/"+hash+".pdf", a.UserID)
				if e != nil {
					return nil, e
				}
				n, e := r.LastInsertId()
				if e != nil {
					return nil, e
				}
				snapshotID = core.WrapID(n)
			} else if e != nil {
				return nil, e
			}
			if e = s.Core.Audit(ctx, tx, a, "invoice.document_reserved", "invoices", id, "pending", map[string]string{"snapshot_id": snapshotID, "kind": string(p.Kind), "invoice_version": fmt.Sprint(p.Version)}); e != nil {
				return nil, e
			}
			return map[string]string{"id": snapshotID}, nil
		})
		if e != nil {
			return e
		}
		raw, e := json.Marshal(reserved)
		if e != nil {
			return e
		}
		var result map[string]string
		if e = json.Unmarshal(raw, &result); e != nil {
			return e
		}
		snap, e = loadDocumentSnapshot(ctx, tx, id, result["id"], true)
		return e
	})
	if e != nil {
		return nil, dbError(e)
	}
	if snap.Status == "available" {
		return snap.result(), nil
	}

	var public PublicDocument
	if e = json.Unmarshal([]byte(snap.PublicData), &public); e != nil {
		return nil, core.Conflict
	}
	data, _, e := RenderDocumentPDF(public)
	if e != nil {
		return nil, e
	}
	checksum := security.Hash(string(data))
	if e = store.PutImmutable(ctx, snap.ObjectKey, data, checksum); e != nil {
		return nil, e
	}

	e = s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "facturas.documents.generate"); e != nil {
			return e
		}
		if e := s.Core.Authorize(ctx, tx, a, "core.documents.write", a.UserID, "", false); e != nil {
			return e
		}
		o, e := s.access(ctx, tx, a, id, "facturas.documents.generate", true)
		if e != nil {
			return e
		}
		snap, e = loadDocumentSnapshot(ctx, tx, id, snap.ID, true)
		if e != nil {
			return e
		}
		if snap.Status == "available" {
			return nil
		}
		binding := GenerateDocumentInput{Kind: snap.Kind, Version: o.Version, PaymentID: snap.PaymentID, PaymentVersion: snap.PaymentVersion}
		current, e := s.projectStoredDocument(ctx, tx, &o, binding)
		if e != nil {
			return e
		}
		currentRaw, e := json.Marshal(current)
		if e != nil {
			return e
		}
		if documentInputHash(currentRaw) != snap.InputHash {
			return core.Conflict
		}
		filename := "factura-" + id + "-" + snap.ID + ".pdf"
		switch snap.Kind {
		case DocumentDeliveryNote:
			filename = "nota-entrega-" + id + "-" + snap.ID + ".pdf"
		case DocumentReceipt:
			filename = "recibo-" + *snap.PaymentID + "-" + snap.ID + ".pdf"
		}
		snap.DocumentID, e = s.Core.RegisterGeneratedFile(ctx, tx, a, "invoices", id, snap.ObjectKey, core.FileRequest{Filename: filename, MediaType: "application/pdf", Size: int64(len(data)), Checksum: checksum})
		if e != nil {
			return e
		}
		if e = core.Changed(tx.ExecContext(ctx, `UPDATE invoice_document_snapshots SET document_id=?,status='available',available_at=UTC_TIMESTAMP(6) WHERE id=? AND invoice_id=? AND status='pending'`, snap.DocumentID, snap.ID, id)); e != nil {
			return e
		}
		if e = s.Core.Audit(ctx, tx, a, "invoice.document_generated", "invoices", id, "success", map[string]string{"snapshot_id": snap.ID, "document_id": snap.DocumentID, "kind": string(snap.Kind)}); e != nil {
			return e
		}
		if e = s.Core.EmitEvent(ctx, tx, a, "invoice.document_generated", "invoices", id, o.Version); e != nil {
			return e
		}
		snap.Status = "available"
		return nil
	})
	if e != nil {
		return nil, dbError(e)
	}
	return snap.result(), nil
}

func nullableDocumentString(v sql.NullString) *string {
	if !v.Valid {
		return nil
	}
	value := v.String
	return &value
}

func (s *Service) GeneratedDocuments(ctx context.Context, a core.Actor, id string, page, perPage int) (any, error) {
	if core.ID(id) != nil || page < 1 || page > 100000 || perPage < 1 || perPage > 100 {
		return nil, core.Invalid
	}
	result := map[string]any{"data": []GeneratedDocument{}, "total": 0, "page": page, "per_page": perPage}
	e := s.Core.Transact(ctx, func(tx *sql.Tx) error {
		if e := s.gate(ctx, tx, a, "facturas.view"); e != nil {
			return e
		}
		if _, e := s.access(ctx, tx, a, id, "facturas.view", false); e != nil {
			return e
		}
		var total int
		if e := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM invoice_document_snapshots WHERE invoice_id=?`, id).Scan(&total); e != nil {
			return e
		}
		rows, e := tx.QueryContext(ctx, `SELECT CAST(id AS CHAR),CAST(document_id AS CHAR),kind,status,CAST(invoice_version AS CHAR),CAST(payment_id AS CHAR),CAST(payment_version AS CHAR),created_at,available_at FROM invoice_document_snapshots WHERE invoice_id=? ORDER BY id DESC LIMIT ? OFFSET ?`, id, perPage, (page-1)*perPage)
		if e != nil {
			return e
		}
		defer rows.Close()
		data := []GeneratedDocument{}
		for rows.Next() {
			var row GeneratedDocument
			var documentID, paymentID, paymentVersion sql.NullString
			var availableAt sql.NullTime
			if e = rows.Scan(&row.ID, &documentID, &row.Kind, &row.Status, &row.InvoiceVersion, &paymentID, &paymentVersion, &row.CreatedAt, &availableAt); e != nil {
				return e
			}
			row.DocumentID = nullableDocumentString(documentID)
			row.PaymentID = nullableDocumentString(paymentID)
			row.PaymentVersion = nullableDocumentString(paymentVersion)
			if availableAt.Valid {
				v := availableAt.Time
				row.AvailableAt = &v
			}
			data = append(data, row)
		}
		if e = rows.Err(); e != nil {
			return e
		}
		result = map[string]any{"data": data, "total": total, "page": page, "per_page": perPage}
		return nil
	})
	return result, e
}
