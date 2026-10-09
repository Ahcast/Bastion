package database

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"embed"
	"fmt"
	"sort"
	"strings"
)

//go:embed migrations/*.sql
var migrations embed.FS

// MySQL DDL autocommits. Failed migrations are marked dirty and require explicit operator repair.
func Migrate(ctx context.Context, db *sql.DB) error {
	conn, err := db.Conn(ctx)
	if err != nil {
		return err
	}
	defer conn.Close()
	var locked int
	if err = conn.QueryRowContext(ctx, "SELECT GET_LOCK('bastion_migrations',30)").Scan(&locked); err != nil || locked != 1 {
		return fmt.Errorf("migration lock: %v", err)
	}
	defer conn.ExecContext(context.Background(), "SELECT RELEASE_LOCK('bastion_migrations')")
	if _, err = conn.ExecContext(ctx, `CREATE TABLE IF NOT EXISTS schema_migrations (name VARCHAR(100) PRIMARY KEY, checksum CHAR(64) NOT NULL, dirty BOOLEAN NOT NULL, applied_at DATETIME(6) NOT NULL) ENGINE=InnoDB`); err != nil {
		return err
	}
	names, err := migrations.ReadDir("migrations")
	if err != nil {
		return err
	}
	sort.Slice(names, func(i, j int) bool { return names[i].Name() < names[j].Name() })
	for _, entry := range names {
		body, err := migrations.ReadFile("migrations/" + entry.Name())
		if err != nil {
			return err
		}
		checksum := fmt.Sprintf("%x", sha256.Sum256(body))
		var previous string
		var dirty bool
		err = conn.QueryRowContext(ctx, "SELECT checksum,dirty FROM schema_migrations WHERE name=?", entry.Name()).Scan(&previous, &dirty)
		if err == nil {
			if dirty || previous != checksum {
				return fmt.Errorf("migration %s dirty or modified", entry.Name())
			}
			continue
		}
		if err != sql.ErrNoRows {
			return err
		}
		if _, err = conn.ExecContext(ctx, "INSERT INTO schema_migrations VALUES(?,?,TRUE,UTC_TIMESTAMP(6))", entry.Name(), checksum); err != nil {
			return err
		}
		for _, statement := range strings.Split(string(body), ";") {
			if strings.TrimSpace(statement) == "" {
				continue
			}
			if _, err = conn.ExecContext(ctx, statement); err != nil {
				return fmt.Errorf("migration %s: %w", entry.Name(), err)
			}
		}
		if _, err = conn.ExecContext(ctx, "UPDATE schema_migrations SET dirty=FALSE,applied_at=UTC_TIMESTAMP(6) WHERE name=?", entry.Name()); err != nil {
			return err
		}
	}
	return nil
}
func Ready(ctx context.Context, db *sql.DB) error {
	if err := db.PingContext(ctx); err != nil {
		return err
	}
	names, _ := migrations.ReadDir("migrations")
	for _, n := range names {
		var checksum string
		var dirty bool
		if err := db.QueryRowContext(ctx, "SELECT checksum,dirty FROM schema_migrations WHERE name=?", n.Name()).Scan(&checksum, &dirty); err != nil {
			return err
		}
		b, _ := migrations.ReadFile("migrations/" + n.Name())
		if dirty || checksum != fmt.Sprintf("%x", sha256.Sum256(b)) {
			return fmt.Errorf("schema mismatch")
		}
	}
	return nil
}
