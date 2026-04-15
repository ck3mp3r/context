package test

import "http"
import "os"

// test_qualified_composite_literal_usage: qualified composite literal
// should produce Usage edge to "http.Server".
func createServer() {
	srv := http.Server{}
	_ = srv
}

// test_qualified_usage_in_short_var: qualified selector in short var
// should produce Usage edge to "os.Stdout".
func processShortVar() {
	x := os.Stdout
	_ = x
}

// test_qualified_usage_in_var_decl: qualified selector in var decl
// should produce Usage edge to "os.Stdout".
func processVarDecl() {
	var out = os.Stdout
	_ = out
}

// test_qualified_usage_in_assign: qualified selector in assignment
// should produce Usage edge to "os.Stdin".
func processAssign() {
	var x interface{}
	x = os.Stdin
	_ = x
}

// test_qualified_usage_in_return: qualified selector in return
// should produce Usage edge to "os.Stdout".
func getStdout() interface{} {
	return os.Stdout
}

// test_qualified_usage_in_call_arg: qualified selector as call argument
// should produce Usage edge to "os.Stdout".
func processCallArg() {
	doSomething(os.Stdout)
}

func doSomething(x interface{}) {}
