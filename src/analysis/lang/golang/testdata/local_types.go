package local_types

// Top-level struct - should be extracted
type Config struct {
	Name string
}

// Top-level function with local type inside
func ParseResponse() string {
	// Local type - should NOT be extracted as a symbol
	type InvokeModelResponseBody struct {
		Completion string
	}
	output := InvokeModelResponseBody{}
	return output.Completion
}

// Another function with same local type name
func ParseOther() string {
	// Different local type with same name - should NOT be extracted
	type InvokeModelResponseBody struct {
		Data string
	}
	output := InvokeModelResponseBody{}
	return output.Data
}

// Top-level struct that references another top-level type
type Response struct {
	Config Config
}
