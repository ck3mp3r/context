package test

// test_interface_method_param_type_refs: Handler.Handle takes Request param
type Request struct{}

type Handler interface {
	Handle(req Request)
}

// test_interface_method_return_type_refs: Responder.Handle returns Response
type Response struct{}

type Responder interface {
	Handle() Response
}

// test_interface_method_builtin_types_skipped: Reader2.Read uses only builtins
// (byte, int, error) — should NOT produce ParamType/ReturnType edges.
type Reader2 interface {
	Read(p []byte) (int, error)
}

// test_interface_method_ptr_param_type_refs: Service.Init takes *Config param
type Config struct{}

type Service interface {
	Init(cfg *Config)
}
