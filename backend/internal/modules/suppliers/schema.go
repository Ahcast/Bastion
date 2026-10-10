package suppliers

import (
	_ "embed"
	"encoding/json"
)

//go:embed schemas.json
var schemas []byte

// Explicit wire schemas preserve BIGINT/version strings in the official SDK.
func outputSchema(name string) any {
	var all map[string]any
	if e := json.Unmarshal(schemas, &all); e != nil {
		panic("invalid supplier schema")
	}
	schema, ok := all[name]
	if !ok {
		panic("missing supplier schema")
	}
	return schema
}
