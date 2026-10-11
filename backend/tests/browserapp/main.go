// Test-only HTTP composition with an in-memory immutable object adapter.
// Never shipped by make build; refuses production and any other database.
package main

import (
	"context"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"sync"
	"time"

	"github.com/Ahcast/Bastion/backend/internal/composition"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"github.com/Ahcast/Bastion/backend/internal/core/api"
	"github.com/Ahcast/Bastion/backend/internal/core/runtime"
	"github.com/Ahcast/Bastion/backend/internal/core/security"
	"github.com/Ahcast/Bastion/backend/internal/modules/quotes"
	"github.com/Ahcast/Bastion/backend/internal/modules/workflow"
	"github.com/go-sql-driver/mysql"
)

type download struct {
	key, name       string
	media, checksum string
	size            int64
	expires         time.Time
}
type objects struct {
	mu    sync.Mutex
	data  map[string][]byte
	links map[string]download
}

func (f *objects) PutImmutable(ctx context.Context, key string, b []byte, hash string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	if security.Hash(string(b)) != hash {
		return core.Invalid
	}
	if old, ok := f.data[key]; ok && security.Hash(string(old)) != hash {
		return core.Conflict
	}
	f.data[key] = append([]byte{}, b...)
	return nil
}
func (f *objects) UploadURL(ctx context.Context, key, media string, size int64, checksum string) (string, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	token := security.ID() + security.ID()
	f.links[token] = download{key: key, media: media, size: size, checksum: checksum, expires: time.Now().Add(5 * time.Minute)}
	return "http://localhost:8080/synthetic-uploads/" + token, nil
}
func (f *objects) upload(w http.ResponseWriter, r *http.Request) {
	if r.Header.Get("Origin") != "http://localhost:4200" {
		http.Error(w, "synthetic origin denied", 403)
		return
	}
	w.Header().Set("Access-Control-Allow-Origin", "http://localhost:4200")
	w.Header().Set("Access-Control-Allow-Methods", "PUT, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, x-amz-checksum-sha256")
	if r.Method == "OPTIONS" {
		w.WriteHeader(204)
		return
	}
	f.mu.Lock()
	defer f.mu.Unlock()
	d, ok := f.links[r.PathValue("token")]
	if !ok || !d.expires.After(time.Now()) || d.size < 1 {
		http.NotFound(w, r)
		return
	}
	b, e := io.ReadAll(http.MaxBytesReader(w, r.Body, 20*1024*1024))
	if e != nil || int64(len(b)) != d.size || r.Header.Get("Content-Type") != d.media || r.Header.Get("x-amz-checksum-sha256") != d.checksum {
		http.Error(w, "synthetic invalid upload", 422)
		return
	}
	raw, _ := hex.DecodeString(security.Hash(string(b)))
	if base64.StdEncoding.EncodeToString(raw) != d.checksum {
		http.Error(w, "synthetic checksum mismatch", 422)
		return
	}
	f.data[d.key] = b
	w.WriteHeader(204)
}
func (f *objects) Info(ctx context.Context, key string) (core.ObjectInfo, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	b, ok := f.data[key]
	if !ok {
		return core.ObjectInfo{}, core.Missing
	}
	h, _ := hex.DecodeString(security.Hash(string(b)))
	return core.ObjectInfo{Size: int64(len(b)), Checksum: base64.StdEncoding.EncodeToString(h), Prefix: b[:min(len(b), 512)]}, nil
}
func (f *objects) DownloadURL(ctx context.Context, key, name string) (string, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	if _, ok := f.data[key]; !ok {
		return "", core.Missing
	}
	token := security.ID() + security.ID()
	f.links[token] = download{key: key, name: name, expires: time.Now().Add(5 * time.Minute)}
	return "http://localhost:8080/synthetic-objects/" + token, nil
}
func (f *objects) serve(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	defer f.mu.Unlock()
	d, ok := f.links[r.PathValue("token")]
	if !ok || !d.expires.After(time.Now()) {
		http.NotFound(w, r)
		return
	}
	w.Header().Set("Content-Type", "application/pdf")
	w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%q", d.name))
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	_, _ = w.Write(f.data[d.key])
}

// This adapter cannot access the network or deliver a real message.
type syntheticMailer struct{}

func (syntheticMailer) Send(ctx context.Context, settings core.SMTPSettings, password string, email core.Email) error {
	return core.ValidateEmail(email)
}
func (f *objects) ReadObject(ctx context.Context, key string, max int64) ([]byte, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	b, ok := f.data[key]
	if !ok || int64(len(b)) > max {
		return nil, core.Missing
	}
	return append([]byte{}, b...), nil
}
func main() {
	logger := slog.New(slog.NewJSONHandler(os.Stdout, nil))
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	s, r, e := runtime.Open(ctx)
	if e != nil {
		logger.Error("synthetic configuration unavailable")
		os.Exit(1)
	}
	defer s.DB.Close()
	d, e := mysql.ParseDSN(s.Config.DSN)
	if e != nil || d.DBName != "bastion_browser_test" || s.Config.Env != "development" || s.Config.Listen != "127.0.0.1:8080" || s.Config.Email || s.Config.MCP {
		logger.Error("isolated browser test configuration required")
		os.Exit(1)
	}
	s.Config.Files = true
	s.Config.Email = true
	s.Config.SMTPHosts = []string{"smtp.example.test"}
	f := &objects{data: map[string][]byte{}, links: map[string]download{}}
	if e = composition.RegisterFiles(s, r, logger, f); e != nil {
		logger.Error("synthetic composition unavailable")
		os.Exit(1)
	}
	q := &quotes.Service{Core: s, Approvals: &workflow.Service{Core: s}}
	w := core.Worker{Service: s, Authorizers: map[string]core.JobHandler{"quote.email.send": q.AuthorizeEmailJob}, Handlers: map[string]core.JobHandler{"quote.email.send": q.CommercialEmailHandler(f, syntheticMailer{}), "notification": s.NotificationHandler}}
	go func() {
		for range time.NewTicker(200 * time.Millisecond).C {
			if e := w.Run(context.Background(), 30); e != nil {
				logger.Error("synthetic worker failed")
			}
		}
	}()
	mux := http.NewServeMux()
	mux.HandleFunc("GET /synthetic-objects/{token}", f.serve)
	mux.HandleFunc("PUT /synthetic-uploads/{token}", f.upload)
	mux.HandleFunc("OPTIONS /synthetic-uploads/{token}", f.upload)
	mux.Handle("/", api.New(s, r, f, logger))
	server := &http.Server{Addr: s.Config.Listen, Handler: mux, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 15 * time.Second, WriteTimeout: 30 * time.Second, IdleTimeout: 60 * time.Second}
	logger.Info("test-only server started")
	if e = server.ListenAndServe(); e != nil {
		logger.Error("test-only server stopped")
		os.Exit(1)
	}
}
