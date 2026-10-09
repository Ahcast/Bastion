package main

import (
	"context"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/composition"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/adapters"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/runtime"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

func main() {
	logger := slog.New(slog.NewJSONHandler(os.Stdout, nil))
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	s, r, e := runtime.Open(ctx)
	cancel()
	if e != nil {
		logger.Error("configuration or database unavailable")
		os.Exit(1)
	}
	defer s.DB.Close()
	if e = composition.Register(s, r, logger); e != nil {
		logger.Error("module configuration unavailable")
		os.Exit(1)
	}
	ctx, cancel = context.WithTimeout(context.Background(), 5*time.Second)
	e = database.Ready(ctx, s.DB)
	cancel()
	if e != nil {
		logger.Error("schema unavailable: run migrations")
		os.Exit(1)
	}
	var objects core.Objects
	if s.Config.Files {
		objects, e = adapters.NewS3(context.Background(), s.Config.Bucket, s.Config.Region)
		if e != nil {
			logger.Error("S3 configuration unavailable")
			os.Exit(1)
		}
	}
	server := &http.Server{Addr: s.Config.Listen, Handler: api.New(s, r, objects, logger), ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 15 * time.Second, WriteTimeout: 30 * time.Second, IdleTimeout: 60 * time.Second, MaxHeaderBytes: 16 * 1024}
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, syscall.SIGTERM, syscall.SIGINT)
	go func() {
		<-stop
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = server.Shutdown(ctx)
	}()
	logger.Info("server started", "listen", s.Config.Listen)
	if e = server.ListenAndServe(); e != nil && e != http.ErrServerClosed {
		logger.Error("server stopped unexpectedly")
		os.Exit(1)
	}
}
