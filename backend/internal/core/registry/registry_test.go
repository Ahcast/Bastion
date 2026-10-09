package registry

import "testing"

func TestDependencies(t *testing.T) {
	r := New()
	_ = r.Register(Module{ID: "core", Label: "Core", Version: "1", Enabled: true})
	_ = r.Register(Module{ID: "a", Label: "A", Version: "1", Enabled: true, Dependencies: []string{"missing"}})
	if r.Validate() == nil {
		t.Fatal("missing dependency")
	}
	if r.Register(Module{ID: "core", Label: "Core", Version: "1"}) == nil {
		t.Fatal("duplicate")
	}
	r = New()
	_ = r.Register(Module{ID: "a", Label: "A", Version: "1", Enabled: true, Dependencies: []string{"b"}})
	_ = r.Register(Module{ID: "b", Label: "B", Version: "1", Enabled: true, Dependencies: []string{"a"}})
	if r.Validate() == nil {
		t.Fatal("cycle")
	}
}
