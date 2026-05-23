package test

// test_usage_edges_for_unexported_types: unexported type identifiers in
// return, short var, and assignment produce Usage edges.
var globalVal myHelper

func getHelper() myHelper {
	return globalVal
}

func setup() {
	h := globalVal
	_ = h
}

func reassign() {
	var x myHelper
	x = globalVal
	_ = x
}

// test_no_usage_edges_for_builtin_types: string/int are builtins —
// should NOT produce Usage edges.
func getName() string {
	return ""
}

func getCount() int {
	return 0
}

// test_binary_expression_usage_edges: MaxSize in binary expression
// should produce a Usage edge.
const MaxSize = 100

func check(x int) bool {
	return x > MaxSize
}

// test_binary_expression_skips_builtins: local vars in binary
// expressions should NOT create builtin Usage edges.
func compare(a int, b int) bool {
	return a > b
}

// test_call_arg_usage_edges: passing a named constant as a call
// argument should produce a Usage edge.
const DefaultTimeout = 30

func setupTimeout() {
	configure(DefaultTimeout)
}

func configure(timeout int) {}
