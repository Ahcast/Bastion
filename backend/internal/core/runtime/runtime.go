package runtime

import (
	"context"
	"database/sql"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/config"
	"github.com/Ahcast/Bastion/backend/internal/core/registry"
	_ "github.com/go-sql-driver/mysql"
	"time"
)

func Open(ctx context.Context) (*core.Service, *registry.Registry, error) {
	c, e := config.Load()
	if e != nil {
		return nil, nil, e
	}
	db, e := sql.Open("mysql", c.DSN)
	if e != nil {
		return nil, nil, e
	}
	db.SetMaxOpenConns(8)
	db.SetMaxIdleConns(4)
	db.SetConnMaxLifetime(3 * time.Minute)
	if e = db.PingContext(ctx); e != nil {
		db.Close()
		return nil, nil, e
	}
	r := registry.New()
	if e = r.Register(registry.Module{ID: "core", Label: "Plataforma", Version: "0.1.0", Enabled: true, Dependencies: []string{}, Permissions: core.Permissions}); e != nil {
		db.Close()
		return nil, nil, e
	}
	return core.New(db, c), r, r.Validate()
}
