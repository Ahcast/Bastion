package api

import (
	"sync"
	"time"

	"golang.org/x/time/rate"
)

type ipLimitEntry struct {
	limiter  *rate.Limiter
	lastSeen time.Time
}

type ipLimiterSet struct {
	mu        sync.Mutex
	entries   map[string]*ipLimitEntry
	limit     rate.Limit
	burst     int
	lastSweep time.Time
}

func newIPLimiterSet(limit rate.Limit, burst int) *ipLimiterSet {
	return &ipLimiterSet{
		entries:   make(map[string]*ipLimitEntry),
		limit:     limit,
		burst:     burst,
		lastSweep: time.Now(),
	}
}

func (l *ipLimiterSet) Allow(ip string) bool {
	if ip == "" {
		ip = "unknown"
	}
	now := time.Now()
	l.mu.Lock()
	defer l.mu.Unlock()

	if now.Sub(l.lastSweep) >= 5*time.Minute {
		for key, entry := range l.entries {
			if now.Sub(entry.lastSeen) > 30*time.Minute {
				delete(l.entries, key)
			}
		}
		l.lastSweep = now
	}

	entry := l.entries[ip]
	if entry == nil {
		entry = &ipLimitEntry{limiter: rate.NewLimiter(l.limit, l.burst)}
		l.entries[ip] = entry
	}
	entry.lastSeen = now
	return entry.limiter.AllowN(now, 1)
}

// Preserve the existing 3000 requests / 15 minute allowance, but keep it in
// process memory and isolate it by the trusted client IP. This removes the
// remote MySQL row lock from the general request path.
var apiIPLimiter = newIPLimiterSet(rate.Limit(3000.0/(15*60)), 3000)
