package core

import "testing"

func TestRecognizedBrowserNames(t *testing.T) {
	for _, tc := range []struct{ ua, want string }{{"Mozilla Windows Chrome/145 Safari/537", "Chrome en Windows"}, {"Mozilla Windows Chrome/146 Safari/537 Edg/146", "Edge en Windows"}, {"Mozilla Macintosh Safari/605", "Safari en macOS"}, {"Mozilla Android Chrome/145", "Chrome en Android"}, {"Mozilla iPhone FxiOS/145", "Firefox en iOS"}, {"", "Navegador en sistema desconocido"}} {
		if BrowserName(tc.ua) != tc.want {
			t.Fatal("wrong browser name")
		}
	}
}
