// Package main creates synthetic data exclusively for the isolated browser test database.
package main

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
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
}
