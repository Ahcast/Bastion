package main

import (
	"database/sql"
	"log"
	"os"
	"strings"
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
	_, e = db.Exec(`INSERT INTO role_permissions
		SELECT r.id,p.name,'all'
		FROM roles r JOIN permissions p
		ON p.name IN ('procesos.view','cotizaciones.view','ordenes.view','facturas.view','core.documents.read','core.documents.write')
		WHERE r.name='Gastos sintético'
		ON DUPLICATE KEY UPDATE scope=VALUES(scope)`)
	if e != nil {
		log.Fatal(e)
	}
}
