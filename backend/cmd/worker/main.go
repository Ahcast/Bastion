package main

import (
	"context"
	"log/slog"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/adapters"
	"github.com/Ahcast/Bastion/backend/internal/core/runtime"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
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

	cleaned, e := s.CleanupExpiredAuthState(ctx)
	if e != nil {
		logger.Error("authentication state cleanup failed")
		os.Exit(1)
	}
	if cleaned.Sessions > 0 || cleaned.AuthLimits > 0 {
		logger.Info("authentication state cleaned", "sessions", cleaned.Sessions, "auth_limits", cleaned.AuthLimits)
	}

	var reader quotes.EmailObjectReader
	if s.Config.Files {
		reader, e = adapters.NewS3(ctx, s.Config.Bucket, s.Config.Region)
		if e != nil {
			logger.Error("private object store unavailable")
			os.Exit(1)
		}
	}
	approvals := &workflow.Service{Core: s}
	quoteService := &quotes.Service{Core: s, Approvals: approvals}
	w := core.Worker{Service: s, Authorizers: map[string]core.JobHandler{"quote.email.send": quoteService.AuthorizeEmailJob}, Handlers: map[string]core.JobHandler{
		"notification":     s.NotificationHandler,
		"email.test":       s.EmailHandler(adapters.SMTP{}),
		"quote.email.send": quoteService.CommercialEmailHandler(reader, adapters.SMTP{}),
	}}
	if e = w.Run(ctx, 30); e != nil && e != context.Canceled && e != context.DeadlineExceeded {
		logger.Error("worker failed")
		os.Exit(1)
	}
}
