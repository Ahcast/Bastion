package suppliers

import (
	"github.com/Ahcast/Bastion/backend/internal/core"
	"net/url"
	"strings"
)

// Decimal validates and canonicalizes an exact nonnegative decimal string.
// No financial arithmetic or rounding is performed when storing observations.
func Decimal(v string, precision, scale int) (string, error) {
	if v == "" {
		return "", nil
	}
	if len(v) > 64 {
		return "", core.Invalid
	}
	parts := strings.Split(v, ".")
	if len(parts) > 2 || parts[0] == "" {
		return "", core.Invalid
	}
	for _, p := range parts {
		if p == "" {
			return "", core.Invalid
		}
		for _, r := range p {
			if r < '0' || r > '9' {
				return "", core.Invalid
			}
		}
	}
	integral := strings.TrimLeft(parts[0], "0")
	if integral == "" {
		integral = "0"
	}
	fraction := ""
	if len(parts) == 2 {
		fraction = parts[1]
	}
	if len(integral) > precision-scale || len(fraction) > scale {
		return "", core.Invalid
	}
	return integral + "." + fraction + strings.Repeat("0", scale-len(fraction)), nil
}
func website(v string, optional bool) bool {
	if v == "" {
		return optional
	}
	if !text(v, 2048, false) || strings.ContainsAny(v, "\r\n\t ") {
		return false
	}
	u, e := url.Parse(v)
	return e == nil && (u.Scheme == "http" || u.Scheme == "https") && u.Hostname() != "" && u.User == nil && u.Opaque == ""
}
