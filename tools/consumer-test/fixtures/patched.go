package fixture

// 0003: Go 1.26 new(expr), and new/make shadowed by a local declaration.
func Fixture() {
	_ = new("hello")
	_ = new(int32(1000))

	var make func(int) *int
	_ = make(1 - 1)

	// the type-taking forms are unchanged
	_ = new(int)
	_ = make([]int, 3)
}
