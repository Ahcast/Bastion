// Package workflow evaluates explicit state graphs. Business modules own the
// graph and transaction; this primitive does not authorize or approve actions.
package workflow

type Edge struct {
	From string
	To   string
}

// Allows denies unknown, identical and undeclared transitions by default.
func Allows(edges []Edge, from, to string) bool {
	if from == "" || to == "" || from == to {
		return false
	}
	for _, edge := range edges {
		if edge.From == from && edge.To == to {
			return true
		}
	}
	return false
}
