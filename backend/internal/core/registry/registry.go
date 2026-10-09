package registry

import (
	"fmt"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"net/http"
	"sort"
)

type Module struct {
	ID           string               `json:"id"`
	Label        string               `json:"label"`
	Version      string               `json:"version"`
	Dependencies []string             `json:"dependencies"`
	Enabled      bool                 `json:"enabled"`
	Permissions  []string             `json:"permissions"`
	Routes       func(*http.ServeMux) `json:"-"`
	Tools        func(*mcp.Server)    `json:"-"`
}
type Registry struct{ modules map[string]Module }

func New() *Registry { return &Registry{map[string]Module{}} }
func (r *Registry) Register(m Module) error {
	if m.ID == "" || m.Label == "" || m.Version == "" {
		return fmt.Errorf("incomplete module")
	}
	if _, ok := r.modules[m.ID]; ok {
		return fmt.Errorf("duplicate module %s", m.ID)
	}
	r.modules[m.ID] = m
	return nil
}
func (r *Registry) Validate() error {
	state := map[string]int{}
	var visit func(string) error
	visit = func(id string) error {
		if state[id] == 1 {
			return fmt.Errorf("module dependency cycle")
		}
		if state[id] == 2 {
			return nil
		}
		m, ok := r.modules[id]
		if !ok || !m.Enabled {
			return fmt.Errorf("missing/disabled module %s", id)
		}
		state[id] = 1
		for _, d := range m.Dependencies {
			if e := visit(d); e != nil {
				return e
			}
		}
		state[id] = 2
		return nil
	}
	for id, m := range r.modules {
		if m.Enabled {
			if e := visit(id); e != nil {
				return e
			}
		}
	}
	return nil
}
func (r *Registry) List() []Module {
	out := []Module{}
	for _, m := range r.modules {
		out = append(out, m)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].ID < out[j].ID })
	return out
}
func (r *Registry) Mount(mux *http.ServeMux) {
	for _, m := range r.List() {
		if m.Enabled && m.Routes != nil {
			m.Routes(mux)
		}
	}
}
func (r *Registry) Tools(server *mcp.Server) {
	for _, m := range r.List() {
		if m.Enabled && m.Tools != nil {
			m.Tools(server)
		}
	}
}
