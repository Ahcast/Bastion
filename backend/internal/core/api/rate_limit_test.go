package api

import (
	"net/http/httptest"
	"testing"

	"golang.org/x/time/rate"
)

func TestClientIPTrustsOnlyLoopbackProxy(t *testing.T) {
	r := httptest.NewRequest("GET", "https://api.example.test/", nil)
	r.RemoteAddr = "127.0.0.1:43210"
	r.Header.Set("X-Real-IP", "203.0.113.42")
	if got := clientIP(r); got != "203.0.113.42" {
		t.Fatalf("trusted proxy IP = %q", got)
	}

	r = httptest.NewRequest("GET", "https://api.example.test/", nil)
	r.RemoteAddr = "198.51.100.10:43210"
	r.Header.Set("X-Real-IP", "203.0.113.99")
	if got := clientIP(r); got != "198.51.100.10" {
		t.Fatalf("untrusted X-Real-IP accepted: %q", got)
	}

	r = httptest.NewRequest("GET", "https://api.example.test/", nil)
	r.RemoteAddr = "[::1]:43210"
	r.Header.Set("X-Real-IP", "2001:db8::42")
	if got := clientIP(r); got != "2001:db8::42" {
		t.Fatalf("trusted IPv6 proxy IP = %q", got)
	}
}

func TestIPLimiterSeparatesClients(t *testing.T) {
	limits := newIPLimiterSet(rate.Limit(1), 1)
	if !limits.Allow("198.51.100.1") {
		t.Fatal("first request unexpectedly rejected")
	}
	if limits.Allow("198.51.100.1") {
		t.Fatal("same client exceeded burst")
	}
	if !limits.Allow("198.51.100.2") {
		t.Fatal("one client exhausted another client's bucket")
	}
}
