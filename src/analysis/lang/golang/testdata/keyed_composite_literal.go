package main

type Inner struct {
	Value string
}

type Outer struct {
	Inner Inner // line 8: field name shadows struct name
}

func create() Outer {
	return Outer{
		Inner: Inner{ // line 13: nested keyed composite literal
			Value: "test",
		},
	}
}

func createVar() {
	var x Outer = Outer{
		Inner: Inner{ // line 20: nested keyed composite literal in var decl
			Value: "test",
		},
	}
	_ = x
}
