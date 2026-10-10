package processes

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"github.com/Ahcast/Bastion/backend/internal/core"
	"net/url"
	"regexp"
	"sort"
	"strings"
	"time"
	"unicode/utf8"
)

var keyPattern = regexp.MustCompile(`^[A-Za-z0-9_-]+$`)
var hashPattern = regexp.MustCompile(`^[a-f0-9]{64}$`)

func key(v string, n int) bool { return len(v) > 0 && len(v) <= n && keyPattern.MatchString(v) }
func digest(b []byte) string   { h := sha256.Sum256(b); return hex.EncodeToString(h[:]) }

// Python crawler dumps uses UTF-8, sorted keys and no HTML escaping.
func canonical(v any) []byte {
	raw, _ := json.Marshal(v)
	var normalized any
	_ = json.Unmarshal(raw, &normalized)
	var b bytes.Buffer
	e := json.NewEncoder(&b)
	e.SetEscapeHTML(false)
	_ = e.Encode(normalized)
	out := bytes.TrimSuffix(b.Bytes(), []byte("\n"))
	var decoded bytes.Buffer
	for i := 0; i < len(out); i++ {
		if out[i] == '\\' && i+6 <= len(out) && (string(out[i:i+6]) == `\u2028` || string(out[i:i+6]) == `\u2029`) {
			n := 0
			for j := i - 1; j >= 0 && out[j] == '\\'; j-- {
				n++
			}
			if n%2 == 0 {
				if out[i+5] == '8' {
					decoded.WriteRune(' ')
				} else {
					decoded.WriteRune(' ')
				}
				i += 5
				continue
			}
		}
		decoded.WriteByte(out[i])
	}
	out = decoded.Bytes()
	return out
}
func hash(v any) string { return digest(canonical(v)) }
func validURL(v string) bool {
	u, e := url.Parse(v)
	return e == nil && (u.Scheme == "http" || u.Scheme == "https") && u.Hostname() != "" && u.User == nil && !strings.ContainsAny(v, "\r\n\t ") && len(v) <= 2048
}
func text(v string, n int) bool {
	return utf8.ValidString(v) && utf8.RuneCountInString(v) <= n && !strings.ContainsRune(v, 0)
}

type Document struct {
	URL    string  `json:"url"`
	Name   string  `json:"name"`
	SHA    *string `json:"sha256,omitempty"`
	Status string  `json:"download_status,omitempty"`
}
type Observation struct {
	Source            string                     `json:"source"`
	ExternalID        string                     `json:"external_id"`
	URL               string                     `json:"source_url"`
	Observed          time.Time                  `json:"observed_at"`
	Fields            []string                   `json:"fields_present"`
	Data              map[string]json.RawMessage `json:"data"`
	DocumentsComplete bool                       `json:"documents_complete"`
	Completeness      string                     `json:"completeness"`
	ContentHash       string                     `json:"content_hash"`
	RawHash           string                     `json:"raw_sha256"`
}
type State struct {
	Fields    map[string]string `json:"fields"`
	Documents []Document        `json:"documents"`
}
type Clocks map[string]time.Time

func emptyState() State { return State{Fields: map[string]string{}, Documents: []Document{}} }

var limits = map[string]int{"entity": 1000, "reference": 500, "deadline": 120, "description": 200000}

func validate(o Observation) error {
	if o.Source != "honducompras" || !text(o.ExternalID, 512) || o.ExternalID == "" || !validURL(o.URL) || o.Observed.Before(time.Date(1970, 1, 1, 0, 0, 0, 0, time.UTC)) || o.Observed.After(time.Now().Add(time.Minute)) || !hashPattern.MatchString(o.ContentHash) || !hashPattern.MatchString(o.RawHash) {
		return core.Invalid
	}
	if o.Completeness != "EMPTY" && o.Completeness != "PARTIAL" && o.Completeness != "COMPLETE" {
		return core.Invalid
	}
	if len(o.Fields) > 4 || o.Fields == nil || o.Data == nil {
		return core.Invalid
	}
	seen := map[string]bool{}
	for _, f := range o.Fields {
		if _, ok := limits[f]; !ok || seen[f] {
			return core.Invalid
		}
		seen[f] = true
		if _, ok := o.Data[f]; !ok {
			return core.Invalid
		}
	}
	for f, b := range o.Data {
		if f == "documents" {
			continue
		}
		n, ok := limits[f]
		var v string
		if !ok || json.Unmarshal(b, &v) != nil || !text(v, n) {
			return core.Invalid
		}
	}
	var docs []Document
	if b, ok := o.Data["documents"]; ok {
		if bytes.Equal(b, []byte("null")) {
			return core.Invalid
		}
		d := json.NewDecoder(bytes.NewReader(b))
		d.DisallowUnknownFields()
		if d.Decode(&docs) != nil || len(docs) > 100 {
			return core.Invalid
		}
	}
	urls := map[string]bool{}
	for _, d := range docs {
		if !validURL(d.URL) || urls[d.URL] || !text(d.Name, 500) || (d.SHA != nil && !hashPattern.MatchString(*d.SHA)) || (d.Status != "" && d.Status != "PENDING" && d.Status != "DOWNLOADED") || (d.Status == "DOWNLOADED" && d.SHA == nil) {
			return core.Invalid
		}
		urls[d.URL] = true
	}
	if o.DocumentsComplete {
		if _, ok := o.Data["documents"]; !ok {
			return core.Invalid
		}
	}
	// Decode through any so maps have canonical key order, preserving null and absent fields.
	var raw any
	if json.Unmarshal(canonical(o.Data), &raw) != nil || hash(raw) != o.ContentHash {
		return core.Invalid
	}
	return nil
}

// Merge returns a new state: caller-owned observations and old snapshots remain immutable.
func Merge(old State, clocks Clocks, o Observation) (State, Clocks, []string, error) {
	if e := validate(o); e != nil {
		return State{}, nil, nil, e
	}
	out := emptyState()
	c := Clocks{}
	for k, v := range old.Fields {
		out.Fields[k] = v
	}
	for k, v := range clocks {
		c[k] = v
	}
	changed := []string{}
	for _, f := range o.Fields {
		var v string
		_ = json.Unmarshal(o.Data[f], &v)
		v = strings.Join(strings.Fields(v), " ")
		t := c[f]
		if o.Observed.Before(t) {
			continue
		}
		if o.Observed.Equal(t) && out.Fields[f] != v {
			return State{}, nil, nil, core.Conflict
		}
		if out.Fields[f] != v {
			out.Fields[f] = v
			changed = append(changed, f)
		}
		c[f] = o.Observed
	}
	docs := map[string]Document{}
	for _, d := range old.Documents {
		d.Status = ""
		docs[d.URL] = d
	}
	var incoming []Document
	if b, ok := o.Data["documents"]; ok {
		_ = json.Unmarshal(b, &incoming)
	}
	seen := map[string]bool{}
	for _, d := range incoming {
		seen[d.URL] = true
		clock := "doc:" + d.URL
		t := c[clock]
		if o.Observed.Before(t) || o.Observed.Before(c["documents"]) {
			continue
		}
		d.Status = ""
		prior, exists := docs[d.URL]
		if d.SHA == nil && exists {
			d.SHA = prior.SHA
		}
		if o.Observed.Equal(t) && hash(d) != hash(prior) {
			return State{}, nil, nil, core.Conflict
		}
		if !exists || hash(d) != hash(prior) {
			changed = append(changed, "document:"+d.URL)
		}
		docs[d.URL] = d
		c[clock] = o.Observed
	}
	if o.DocumentsComplete && !o.Observed.Before(c["documents"]) {
		for u := range docs {
			if !seen[u] && !o.Observed.Before(c["doc:"+u]) {
				if o.Observed.Equal(c["doc:"+u]) {
					return State{}, nil, nil, core.Conflict
				}
				delete(docs, u)
				c["doc:"+u] = o.Observed
				changed = append(changed, "document_removed:"+u)
			}
		}
		c["documents"] = o.Observed
	}
	for _, d := range docs {
		out.Documents = append(out.Documents, d)
	}
	sort.Slice(out.Documents, func(i, j int) bool { return out.Documents[i].URL < out.Documents[j].URL })
	sort.Strings(changed)
	return out, c, changed, nil
}
