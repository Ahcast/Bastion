package main

import (
	"database/sql"
	"log"
	"os"
	"strings"
	"time"
)

// Test-only capabilities for the Gastos Unit 2 browser acceptance. Production
// bootstrap grants none; this binary is built only for the isolated browser DB.
func init() {
	dsn := os.Getenv("BASTION_MYSQL_DSN")
	if !strings.Contains(dsn, "/bastion_browser_test?") {
		return
	}
	db, e := sql.Open("mysql", dsn)
	if e != nil {
		log.Fatal(e)
	}
	defer db.Close()

	// Keep the second Gastos browser scenario on a separate identity so the
	// TOTP anti-replay protection remains active when both tests share a step.
	_, e = db.Exec(`INSERT INTO users(email,name,password_hash,totp_secret,active,created_at,updated_at)
		SELECT 'expense-links@example.test','Operador Gastos Unidad 2 sintético',password_hash,totp_secret,TRUE,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)
		FROM users
		WHERE email='expenses@example.test'
		  AND NOT EXISTS (SELECT 1 FROM users WHERE email='expense-links@example.test')`)
	if e != nil {
		log.Fatal(e)
	}
	_, e = db.Exec(`INSERT IGNORE INTO user_roles
		SELECT u.id,r.id
		FROM users u JOIN roles r ON r.name='Gastos sintético'
		WHERE u.email='expense-links@example.test'`)
	if e != nil {
		log.Fatal(e)
	}

	_, e = db.Exec(`INSERT INTO role_permissions
		SELECT r.id,p.name,'all'
		FROM roles r JOIN permissions p
		ON p.name IN ('procesos.view','cotizaciones.view','ordenes.view','facturas.view','core.documents.read','core.documents.write')
		WHERE r.name='Gastos sintético'
		ON DUPLICATE KEY UPDATE scope=VALUES(scope)`)
	if e != nil {
		log.Fatal(e)
	}

	go rotateExpenseBrowserIdentity(dsn)
}

func rotateExpenseBrowserIdentity(dsn string) {
	db, e := sql.Open("mysql", dsn)
	if e != nil {
		log.Printf("expense browser identity watcher unavailable: %v", e)
		return
	}
	defer db.Close()
	ticker := time.NewTicker(25 * time.Millisecond)
	defer ticker.Stop()
	deadline := time.NewTimer(10 * time.Minute)
	defer deadline.Stop()
	for {
		select {
		case <-deadline.C:
			return
		case <-ticker.C:
			var step int64
			e = db.QueryRow(`SELECT totp_last_step FROM users WHERE name='Operador Gastos sintético' LIMIT 1`).Scan(&step)
			if e != nil || step == 0 {
				continue
			}
			tx, e := db.Begin()
			if e != nil {
				continue
			}
			if _, e = tx.Exec(`UPDATE users SET email='expenses-unit1-complete@example.test' WHERE name='Operador Gastos sintético'`); e != nil {
				_ = tx.Rollback()
				continue
			}
			if _, e = tx.Exec(`UPDATE users SET email='expenses@example.test' WHERE email='expense-links@example.test'`); e != nil {
				_ = tx.Rollback()
				continue
			}
			if e = tx.Commit(); e == nil {
				return
			}
		}
	}
}
