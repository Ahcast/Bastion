// Package main creates synthetic data exclusively for the isolated browser test database.
package main

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
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
	s := core.New(db, config.Config{Key: make([]byte, 32)})
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
	if _, e = db.Exec(`INSERT INTO role_permissions SELECT 7,name,'all' FROM permissions WHERE name IN ('ordenes.view','ordenes.create','ordenes.update','ordenes.transition','ordenes.items.transition','ordenes.deliver','ordenes.documents.write','core.documents.read','core.documents.write')`); e != nil {
		log.Fatal(e)
	}
	if _, e = db.Exec(`INSERT INTO user_roles SELECT id,7 FROM users WHERE email='purchase-orders@example.test'`); e != nil {
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

}
