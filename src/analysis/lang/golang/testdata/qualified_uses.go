package main

import "github.com/example/auth"

var rootCmd = &Command{}

func init() {
	rootCmd.AddCommand(auth.AuthCmd) // line 8: auth.AuthCmd is qualified reference
}

func GetTimeout() int {
	return auth.DefaultTimeout // line 12: qualified const reference inside function
}
