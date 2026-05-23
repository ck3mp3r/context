package field_type_resolution

// Struct type named Config
type Config struct {
	Name string
}

// Another struct with a field named "Config" (same name as the type)
type Settings struct {
	Config string // field named Config, type is string
}

// Struct that references Config TYPE, not the field
type Server struct {
	Config Config // field named Config, type is Config struct
}
