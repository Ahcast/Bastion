package main

import (
	"database/sql"
	"log"
	"os"
	"strings"
)

// Test-only grants and identity for Collaboration U3/U4 browser acceptance.
// Production bootstrap grants none and this binary refuses any non-browser DB.
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
	_, e = db.Exec(`INSERT INTO role_permissions
		SELECT r.id,p.name,'all'
		FROM roles r JOIN permissions p ON p.name IN (
			'colaboracion.task','colaboracion.link',
			'procesos.view','cotizaciones.view','ordenes.view','facturas.view'
		)
		WHERE r.name='Colaboración sintético'
		ON DUPLICATE KEY UPDATE scope=VALUES(scope)`)
	if e != nil {
		log.Fatal(e)
	}
	_, e = db.Exec(`INSERT INTO role_permissions
		SELECT r.id,p.name,'all'
		FROM roles r JOIN permissions p ON p.name IN ('colaboracion.task')
		WHERE r.name='Colaboración revisor sintético'
		ON DUPLICATE KEY UPDATE scope=VALUES(scope)`)
	if e != nil {
		log.Fatal(e)
	}

	// A separate user avoids TOTP anti-replay collisions with the U1/U2 spec.
	_, e = db.Exec(`INSERT INTO users(email,name,password_hash,active,totp_secret,totp_pending,totp_last_step,version,created_at,updated_at)
		SELECT 'collaboration-work@example.test','Operador Colaboración trabajo sintético',password_hash,active,totp_secret,totp_pending,-1,1,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6)
		FROM users WHERE email='collaboration@example.test'
		ON DUPLICATE KEY UPDATE name=VALUES(name),password_hash=VALUES(password_hash),active=VALUES(active),totp_secret=VALUES(totp_secret),totp_pending=VALUES(totp_pending),totp_last_step=-1,updated_at=UTC_TIMESTAMP(6)`)
	if e != nil {
		log.Fatal(e)
	}
	_, e = db.Exec(`INSERT IGNORE INTO recovery_codes(user_id,code_hash)
		SELECT work.id,rc.code_hash FROM users source
		JOIN users work ON work.email='collaboration-work@example.test'
		JOIN recovery_codes rc ON rc.user_id=source.id
		WHERE source.email='collaboration@example.test'`)
	if e != nil {
		log.Fatal(e)
	}
	_, e = db.Exec(`INSERT IGNORE INTO user_roles(user_id,role_id)
		SELECT u.id,r.id FROM users u JOIN roles r ON r.name='Colaboración sintético'
		WHERE u.email='collaboration-work@example.test'`)
	if e != nil {
		log.Fatal(e)
	}
}
