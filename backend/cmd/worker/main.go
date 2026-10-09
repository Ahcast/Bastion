package main

import (
	"context"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/adapters"
	"github.com/Ahcast/Bastion/backend/internal/core/runtime"
	"log/slog"
	"os"
	"os/signal"
	"syscall"
	"time"
)

func main() {
	logger := slog.New(slog.NewJSONHandler(os.Stdout, nil))
	ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGTERM, syscall.SIGINT)
	defer cancel()
	ctx, budget := context.WithTimeout(ctx, 60*time.Second)
	defer budget()
	s, _, e := runtime.Open(ctx)
	if e != nil {
		logger.Error("configuration or database unavailable")
		os.Exit(1)
	}
	defer s.DB.Close()
	if e = database.Ready(ctx, s.DB); e != nil {
		logger.Error("schema unavailable")
		os.Exit(1)
	}
	w := core.Worker{Service: s, Handlers: map[string]core.JobHandler{"notification": s.NotificationHandler, "email.test": s.EmailHandler(adapters.SMTP{})}}
	if e = w.Run(ctx, 30); e != nil && e != context.Canceled && e != context.DeadlineExceeded {
		logger.Error("worker failed")
		os.Exit(1)
	}
}
