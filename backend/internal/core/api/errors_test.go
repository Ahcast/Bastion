package api

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"github.com/go-sql-driver/mysql"
	"log/slog"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestFailureDiagnosticsExposeCodesWithoutSensitiveMessages(t *testing.T) {
	for _, e := range []error{&mysql.MySQLError{Number: 1213, Message: "private SQL and secret"}, fmt.Errorf("private detail: %w", context.Canceled)} {
		var log bytes.Buffer
		s := &Server{Logger: slog.New(slog.NewJSONHandler(&log, nil))}
		w := httptest.NewRecorder()
		s.error(w, httptest.NewRequest("GET", "/api/v1/quotes/1", nil), e)
		if w.Code != 500 || strings.Contains(log.String(), "private") || strings.Contains(w.Body.String(), "private") {
			t.Fatal(log.String(), w.Body.String())
		}
		var record map[string]any
		if err := json.Unmarshal(log.Bytes(), &record); err != nil {
			t.Fatal(err)
		}
		if _, ok := e.(*mysql.MySQLError); ok {
			if record["database_error_number"] != float64(1213) {
				t.Fatal(record)
			}
		} else if record["context_canceled"] != true {
			t.Fatal(record)
		}
	}
}
