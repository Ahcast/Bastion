// Package main creates synthetic data exclusively for the isolated browser test database.
package main

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/purchaseorders"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/go-sql-driver/mysql"
	"log"
	"os"
	"strings"
	"time"
)

func main() {
	dsn := os.Getenv("BASTION_TEST_MYSQL_DSN")
	if !strings.Contains(dsn, "/bastion_browser_test?") {
		log.Fatal("dedicated bastion_browser_test database required")
	}
	cfg, e := mysql.ParseDSN(dsn)
	if e != nil {
		log.Fatal(e)
	}
	cfg.DBName = ""
	root, e := sql.Open("mysql", cfg.FormatDSN())
	if e != nil {
		log.Fatal(e)
	}
	if _, e = root.Exec("CREATE DATABASE IF NOT EXISTS bastion_browser_test CHARACTER SET utf8mb4"); e != nil {
		log.Fatal(e)
	}
	root.Close()
	db, e := sql.Open("mysql", dsn)
	if e != nil {
		log.Fatal(e)
	}
	defer db.Close()
	if e = database.Migrate(context.Background(), db); e != nil {
		log.Fatal(e)
	}
	s := core.New(db, config.Config{Key: make([]byte, 32), Idle: 30 * time.Minute, Absolute: 12 * time.Hour, Reauth: 5 * time.Minute})
	if e = s.Bootstrap(context.Background(), "browser@example.test", "Administrador sintético", "synthetic browser password 42"); e != nil {
		log.Fatal(e)
	}
	// This synthetic employee has explicitly seeded customer permissions. Production bootstrap grants none.
	secret, e := security.Encrypt(s.Config.Key, "JBSWY3DPEHPK3PXP")
	if e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'customers@example.test','Operador Clientes sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(id,name) VALUES(2,'Clientes sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 2,name,'all' FROM permissions WHERE name IN ('clientes.view','clientes.create','clientes.update')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(2,2)`); e != nil {
		log.Fatal(e)
	}
	// A separate synthetic employee is explicitly granted supplier permissions only in this test schema.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'suppliers@example.test','Operador Proveedores sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(id,name) VALUES(3,'Proveedores sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 3,name,'all' FROM permissions WHERE name IN ('proveedores.view','proveedores.create','proveedores.update')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(3,3)`); e != nil {
		log.Fatal(e)
	}

	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'processes@example.test','Operador Procesos sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(id,name) VALUES(4,'Procesos sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 4,name,'all' FROM permissions WHERE name LIKE 'procesos.%'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(4,4)`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO processes(id,source,external_id,source_url,entity,reference,description,deadline,normalized_data,field_clocks,content_hash,created_by,first_seen_at,last_seen_at,version) VALUES(1,'honducompras','synthetic-browser','https://example.test/process','Entidad sintética','HC-TEST-001','Especificaciones sintéticas','',?, '{}',?,4,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6),1)`, `{"fields":{"entity":"Entidad sintética","reference":"HC-TEST-001","description":"Especificaciones sintéticas"},"documents":[]}`, security.Hash("synthetic")); e != nil {
		log.Fatal(e)
	}

	if _, e = db.Exec(`INSERT INTO crawler_runs VALUES('HC-BROWSER-TEST','honducompras',UTC_TIMESTAMP(6),4,UTC_TIMESTAMP(6))`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO process_versions(process_id,crawler_run_id,version,content_hash,normalized_data,changes,observed_at,created_at) SELECT id,'HC-BROWSER-TEST',version,content_hash,normalized_data,'[]',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM processes WHERE id=1`); e != nil {
		log.Fatal(e)
	}

	// Quote operator exists only in the isolated browser schema; production grants none.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'quotes@example.test','Operador Cotizaciones sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(id,name) VALUES(5,'Cotizaciones sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 5,name,'all' FROM permissions WHERE name LIKE 'cotizaciones.%' OR name IN ('procesos.view','core.documents.read','core.documents.write')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(5,5)`); e != nil {
		log.Fatal(e)
	}

	// Dedicated FX operator prevents browser TOTP replay between independent quote scenarios.
	// It has only the global FX grants required by the observation-ledger screen.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'fx@example.test','Operador FX sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(id,name) VALUES(6,'FX sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 6,name,'all' FROM permissions WHERE name IN ('cotizaciones.fx.view','cotizaciones.fx.manage')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(6,6)`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO recovery_codes VALUES(6,?)`, security.Hash("synthetic-fx-recovery")); e != nil {
		log.Fatal(e)
	}

	// Dedicated customer purchase-order operator. It has no customer/supplier catalog grants:
	// this browser scenario proves Unit 1 can operate with authorized manual snapshots.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'purchase-orders@example.test','Operador Órdenes sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(id,name) VALUES(7,'Órdenes sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 7,name,'all' FROM permissions WHERE name IN ('ordenes.view','ordenes.create','ordenes.update','ordenes.transition','ordenes.items.transition','ordenes.deliver','ordenes.documents.write','ordenes.delete','core.documents.read','core.documents.write')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT id,7 FROM users WHERE email='purchase-orders@example.test'`); e != nil {
		log.Fatal(e)
	}

	// The Collaboration browser actor can upload and manage its own synthetic library only.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'collaboration@example.test','Operador Colaboración sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Colaboración sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'all' FROM roles r JOIN permissions p ON p.name IN ('colaboracion.view','colaboracion.upload','colaboracion.organize','colaboracion.delete','core.documents.read','core.documents.write') WHERE r.name='Colaboración sintético'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Colaboración sintético' WHERE u.email='collaboration@example.test'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO recovery_codes SELECT id,? FROM users WHERE email='collaboration@example.test'`, security.Hash("synthetic-collaboration-recovery")); e != nil {
		log.Fatal(e)
	}

	if _, e = db.Exec(`INSERT INTO recovery_codes SELECT id,? FROM users WHERE email='purchase-orders@example.test'`, security.Hash("synthetic-po-reauth-recovery")); e != nil {
		log.Fatal(e)
	}

	// Synthetic quarantined metadata only: no S3 request, bytes or scanner bypass.
	if _, e = db.Exec(`INSERT INTO documents(id,owner_id,created_at) VALUES(900,5,UTC_TIMESTAMP(6))`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO document_versions VALUES(900,1,'synthetic-browser/attachment.pdf','adjunto-sintetico.pdf','application/pdf',9,?,'quarantined',NULL,UTC_TIMESTAMP(6))`, security.Hash("synthetic")); e != nil {
		log.Fatal(e)
	}

	smtpSecret, e := security.Encrypt(s.Config.Key, "synthetic-password")
	if e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO settings(name,value,secret,updated_at) VALUES('smtp',?,?,UTC_TIMESTAMP(6))`, `{"host":"smtp.example.test","port":587,"username":"synthetic","from":"ventas@example.test","tls":"starttls"}`, smtpSecret); e != nil {
		log.Fatal(e)
	}
	// Two independent people exercise human approval; no production grants.
	for _, email := range []string{"approval-requester@example.test", "approval-reviewer@example.test", "emission-requester@example.test", "emission-reviewer@example.test"} {
		r, e := db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT ?, 'Aprobación sintética',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, email, secret)
		if e != nil {
			log.Fatal(e)
		}
		id, e := r.LastInsertId()
		if e != nil {
			log.Fatal(e)
		}
		if _, e = db.Exec(`INSERT INTO user_roles VALUES(?,5)`, id); e != nil {
			log.Fatal(e)
		}
		if _, e = db.Exec(`INSERT INTO recovery_codes VALUES(?,?)`, id, security.Hash("synthetic-approval-recovery")); e != nil {
			log.Fatal(e)
		}
	}

	// Dedicated accepted source for conversion UI; test database only.
	result, e := db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'conversion@example.test','Operador conversión sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret)
	if e != nil {
		log.Fatal(e)
	}
	user, e := result.LastInsertId()
	if e != nil {
		log.Fatal(e)
	}
	result, e = db.Exec(`INSERT INTO roles(name) VALUES('Conversión sintética')`)
	if e != nil {
		log.Fatal(e)
	}
	role, e := result.LastInsertId()
	if e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT ?,name,'self' FROM permissions WHERE name IN ('cotizaciones.view','cotizaciones.create','cotizaciones.transition','cotizaciones.convert','ordenes.view','ordenes.create')`, role); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(?,?)`, user, role); e != nil {
		log.Fatal(e)
	}
	s.Config.Idle = 30 * time.Minute
	s.Config.Absolute = 12 * time.Hour
	token, _, e := s.NewSession(context.Background(), core.WrapID(user), true)
	if e != nil {
		log.Fatal(e)
	}
	actor := core.Actor{Type: "user", ID: core.WrapID(user), UserID: core.WrapID(user), SessionHash: security.Hash(token), Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
	quoteService := &quotes.Service{Core: s}
	v, e := quoteService.Save(context.Background(), actor, "", "browser-conversion-source", quotes.Input{Customer: quotes.CustomerInput{Name: "Cliente conversión sintético"}, Date: "2026-10-11", ValidityDays: 30, Currency: "USD", Notes: "Nota privada no copiada", CalculationInput: quotes.CalculationInput{Items: []quotes.Item{{Description: "Equipo conversión sintético", Unit: "unidad", Quantity: "2", Price: "100", Cost: "40", DiscountType: "amount", Discount: "10", Tax: "isv15"}}, DiscountType: "amount", Discount: "5"}})
	if e != nil {
		log.Fatal(e)
	}
	quoteID := v.(map[string]string)["id"]
	if _, e = db.Exec(`UPDATE quotes SET status='sent' WHERE id=?`, quoteID); e != nil {
		log.Fatal(e)
	}
	if _, e = quoteService.Transition(context.Background(), actor, quoteID, "browser-conversion-accepted", quotes.TransitionInput{Status: "accepted", Version: 1, Reason: "Aceptación sintética del cliente"}); e != nil {
		log.Fatal(e)
	}
	// Setup capabilities are removed before the browser logs in.
	if _, e = db.Exec(`DELETE FROM role_permissions WHERE role_id=? AND permission IN ('cotizaciones.create','cotizaciones.transition')`, role); e != nil {
		log.Fatal(e)
	}

	// Invoice operator has explicit draft grants only; no fiscal/payment authority.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'invoices@example.test','Operador Facturas sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Facturas borradores sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'all' FROM roles r JOIN permissions p ON p.name IN ('facturas.view','facturas.create','facturas.update') WHERE r.name='Facturas borradores sintético'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Facturas borradores sintético' WHERE u.email='invoices@example.test'`); e != nil {
		log.Fatal(e)
	}

	// Separate synthetic configuration operator, without invoice draft/emission/payment writes.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'fiscal@example.test','Configuración fiscal sintética',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Facturas configuración sintética')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'all' FROM roles r JOIN permissions p ON p.name IN ('facturas.view','facturas.fiscal.read','facturas.fiscal.manage') WHERE r.name='Facturas configuración sintética'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Facturas configuración sintética' WHERE u.email='fiscal@example.test'`); e != nil {
		log.Fatal(e)
	}

	if _, e = db.Exec(`INSERT INTO recovery_codes SELECT id,? FROM users WHERE email='fiscal@example.test'`, security.Hash("synthetic-fiscal-recovery")); e != nil {
		log.Fatal(e)
	}

	// Separate synthetic issuer/payment operator with self scope, without fiscal-settings administration.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'invoice-emitter@example.test','Emisor factura sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Facturas emisión sintética')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'self' FROM roles r JOIN permissions p ON p.name IN ('facturas.view','facturas.create','facturas.update','facturas.emit','facturas.payment','facturas.payment.void','facturas.documents.generate','core.documents.read','core.documents.write') WHERE r.name='Facturas emisión sintética'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Facturas emisión sintética' WHERE u.email='invoice-emitter@example.test'`); e != nil {
		log.Fatal(e)
	}
	// Independent cancellation operator keeps the existing emission test's
	// correlativo and MFA session isolated. Production grants remain unchanged.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'invoice-cancel@example.test','Anulación factura sintética',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Facturas anulación sintética')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'self' FROM roles r JOIN permissions p ON p.name IN ('facturas.view','facturas.create','facturas.emit','facturas.cancel','facturas.documents.generate','core.documents.read','core.documents.write') WHERE r.name='Facturas anulación sintética'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Facturas anulación sintética' WHERE u.email='invoice-cancel@example.test'`); e != nil {
		log.Fatal(e)
	}

	// Company reviewer is isolated from the administrator's MFA enrollment scenario.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'invoice-company@example.test','Empresa emisión sintética',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Empresa emisión sintética')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'all' FROM roles r JOIN permissions p ON p.name IN ('core.settings.read','core.settings.manage') WHERE r.name='Empresa emisión sintética'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Empresa emisión sintética' WHERE u.email='invoice-company@example.test'`); e != nil {
		log.Fatal(e)
	}

	// Isolated accepted quote for invoice conversion; setup grants are removed before login.
	result, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'quote-invoice@example.test','Conversión factura sintética',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret)
	if e != nil {
		log.Fatal(e)
	}
	user, e = result.LastInsertId()
	if e != nil {
		log.Fatal(e)
	}
	result, e = db.Exec(`INSERT INTO roles(name) VALUES('Conversión factura sintética')`)
	if e != nil {
		log.Fatal(e)
	}
	role, e = result.LastInsertId()
	if e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT ?,name,'self' FROM permissions WHERE name IN ('cotizaciones.view','cotizaciones.create','cotizaciones.transition','cotizaciones.convert','facturas.view','facturas.create','facturas.update')`, role); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(?,?)`, user, role); e != nil {
		log.Fatal(e)
	}
	token, _, e = s.NewSession(context.Background(), core.WrapID(user), true)
	if e != nil {
		log.Fatal(e)
	}
	actor = core.Actor{Type: "user", ID: core.WrapID(user), UserID: core.WrapID(user), SessionHash: security.Hash(token), Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
	v, e = quoteService.Save(context.Background(), actor, "", "browser-invoice-source", quotes.Input{Customer: quotes.CustomerInput{Name: "Cliente factura derivada sintético"}, Date: time.Now().In(time.FixedZone("Honduras", -6*3600)).Format("2006-01-02"), ValidityDays: 30, Currency: "USD", Notes: "Nota privada no copiar", CalculationInput: quotes.CalculationInput{Items: []quotes.Item{{Description: "Equipo factura derivada", Unit: "unidad", Quantity: "2", Price: "100", Cost: "40", DiscountType: "amount", Discount: "10", Tax: "isv15"}}, DiscountType: "amount", Discount: "5"}})
	if e != nil {
		log.Fatal(e)
	}
	quoteID = v.(map[string]string)["id"]
	if _, e = db.Exec(`UPDATE quotes SET status='sent' WHERE id=?`, quoteID); e != nil {
		log.Fatal(e)
	}
	if _, e = quoteService.Transition(context.Background(), actor, quoteID, "browser-invoice-accepted", quotes.TransitionInput{Status: "accepted", Version: 1, Reason: "Aceptación sintética para factura"}); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`DELETE FROM role_permissions WHERE role_id=? AND permission IN ('cotizaciones.create','cotizaciones.transition')`, role); e != nil {
		log.Fatal(e)
	}

	// Delivered OC for billing UI; setup uses real owner transitions and then removes setup grants.
	result, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'order-invoice@example.test','Facturación OC sintética',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret)
	if e != nil {
		log.Fatal(e)
	}
	user, e = result.LastInsertId()
	if e != nil {
		log.Fatal(e)
	}
	result, e = db.Exec(`INSERT INTO roles(name) VALUES('Facturación OC sintética')`)
	if e != nil {
		log.Fatal(e)
	}
	role, e = result.LastInsertId()
	if e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT ?,name,'self' FROM permissions WHERE name IN ('ordenes.view','ordenes.create','ordenes.transition','ordenes.items.transition','ordenes.deliver','ordenes.convert','ordenes.close','ordenes.reopen','facturas.view','facturas.create','facturas.emit','facturas.payment','facturas.payment.void')`, role); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles VALUES(?,?)`, user, role); e != nil {
		log.Fatal(e)
	}
	token, _, e = s.NewSession(context.Background(), core.WrapID(user), true)
	if e != nil {
		log.Fatal(e)
	}
	actor = core.Actor{Type: "user", ID: core.WrapID(user), UserID: core.WrapID(user), SessionHash: security.Hash(token), Source: "web", RequestID: security.ID(), CorrelationID: security.ID()}
	if _, e = db.Exec(`UPDATE sessions SET reauthenticated_at=UTC_TIMESTAMP(6) WHERE token_hash=?`, actor.SessionHash); e != nil {
		log.Fatal(e)
	}
	orderService := &purchaseorders.Service{Core: s}
	v, e = orderService.Save(context.Background(), actor, "", "browser-billing-order", purchaseorders.Input{OrderNumber: "OC-FACTURA-BROWSER", Customer: purchaseorders.CustomerInput{Name: "Cliente OC facturada sintético"}, OrderDate: time.Now().In(time.FixedZone("Honduras", -6*3600)).Format("2006-01-02"), Currency: "HNL", CalculationInput: purchaseorders.CalculationInput{Items: []purchaseorders.Item{{Description: "Equipo OC facturada", Unit: "unidad", Quantity: "1", UnitSalePrice: "100", UnitCost: "40", DiscountType: "amount", Discount: "0", TaxType: "isv15"}}, DiscountType: "amount", Discount: "0", AdditionalExpenses: "0"}})
	if e != nil {
		log.Fatal(e)
	}
	orderID := v.(map[string]string)["id"]
	step := func(position int, to string) {
		o, e := orderService.Get(context.Background(), actor, orderID)
		if e != nil {
			log.Fatal(e)
		}
		if _, e = orderService.Transition(context.Background(), actor, orderID, "browser-billing-step-"+core.WrapID(int64(o.Version)), position, purchaseorders.TransitionInput{Version: o.Version, ToStatus: to}); e != nil {
			log.Fatal(e)
		}
	}
	for _, to := range []string{"purchased", "shipped", "received", "delivered"} {
		step(1, to)
	}
	for _, to := range []string{"confirmed", "purchasing", "in_transit", "goods_received", "preparing_delivery", "delivered"} {
		step(0, to)
	}
	if _, e = db.Exec(`DELETE FROM role_permissions WHERE role_id=? AND permission IN ('ordenes.create','ordenes.transition','ordenes.items.transition','ordenes.deliver')`, role); e != nil {
		log.Fatal(e)
	}

	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'expenses@example.test','Operador Gastos sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Gastos sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'all' FROM roles r JOIN permissions p ON p.name LIKE 'gastos.%' WHERE r.name='Gastos sintético'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Gastos sintético' WHERE u.email='expenses@example.test'`); e != nil {
		log.Fatal(e)
	}

	// Report operator is isolated from commercial mutating browser scenarios.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'reports@example.test','Operador Reportes sintético',password_hash,?,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE id=1`, secret); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO roles(name) VALUES('Reportes sintético')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT r.id,p.name,'all' FROM roles r JOIN permissions p ON p.name IN ('reportes.view','reportes.export','facturas.view','ordenes.view','gastos.view') WHERE r.name='Reportes sintético'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Reportes sintético' WHERE u.email='reports@example.test'`); e != nil {
		log.Fatal(e)
	}

	// Fiscal emission uses an independent MFA identity to avoid replaying a prior test's TOTP step.
	if _, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at) SELECT 'invoice-emission-fiscal@example.test','Fiscal emisión sintético',password_hash,totp_secret,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6) FROM users WHERE email='fiscal@example.test'`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT target.id,ur.role_id FROM users target JOIN users source ON source.email='fiscal@example.test' JOIN user_roles ur ON ur.user_id=source.id WHERE target.email='invoice-emission-fiscal@example.test'`); e != nil {
		log.Fatal(e)
	}

	if _, e = db.Exec(`INSERT INTO recovery_codes SELECT id,? FROM users WHERE email='reports@example.test'`, security.Hash("synthetic-reports-recovery")); e != nil {
		log.Fatal(e)
	}

}
