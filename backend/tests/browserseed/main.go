// Package main creates synthetic data exclusively for the isolated browser test database.
package main

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/go-sql-driver/mysql"
	"log"
	"os"
	"strings"
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

}
