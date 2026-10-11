package finance

import "testing"

func TestProfitExcludesTaxesAndAllowsLossWithoutFloat(t *testing.T) {
	for _, x := range []struct {
		bases            [4]string
		investment, want string
	}{
		{[4]string{"171.00", "171.00", "171.00", "171.00"}, "250.03", "433.97"},
		{[4]string{"175.00", "0", "0", "0"}, "83.00", "92.00"},
		{[4]string{"0", "0", "0", "0"}, "9.99", "-9.99"},
		{[4]string{"0.01", "0.02", "0.03", "0.04"}, "0.11", "-0.01"},
	} {
		v, e := Profit(x.bases[0], x.bases[1], x.bases[2], x.bases[3], x.investment)
		if e != nil || v != x.want {
			t.Fatal(v, e, x.want)
		}
	}
	for _, value := range []string{"-1", "0.001", "1e2", "", "99999999999999999.00"} {
		if _, e := Profit(value, "0", "0", "0", "0"); e == nil {
			t.Fatal(value)
		}
	}
}
