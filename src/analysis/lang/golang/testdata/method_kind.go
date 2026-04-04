package method_kind

// Struct with methods
type Cache struct {
	data map[string]string
}

// Struct method with pointer receiver
func (c *Cache) Get(key string) string {
	return c.data[key]
}

// Struct method with value receiver
func (c Cache) Size() int {
	return len(c.data)
}

// Interface with methods
type Cacher interface {
	Get(key string) string
	Set(key string, value string)
}

// Free function (not a method)
func NewCache() *Cache {
	return &Cache{data: make(map[string]string)}
}

// Another free function
func HelperFunc() {}

// --- HasMethod edge test cases ---

// Multiple methods on same struct
type Server struct {
	host string
	port int
}

func (s *Server) Start() error {
	return nil
}

func (s *Server) Stop() error {
	return nil
}

func (s Server) Address() string {
	return s.host
}
