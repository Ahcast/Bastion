package main

import (
	"database/sql"
	"log"
	"os"
	"strings"
)

// Test-only grants for Collaboration U3/U4 browser acceptance. Production
// bootstrap grants none and this binary refuses any non-browser database.
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
}
