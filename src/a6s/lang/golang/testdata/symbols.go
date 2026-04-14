// Test file for Go symbol extraction
package testpkg

// Exported constant
const MaxSize = 100

// unexported constant
const minSize = 10

// Exported variable
var GlobalCounter int

// unexported variable
var localCache map[string]string

// Exported type alias
type UserID string

// Exported struct
type Server struct {
	Port int      // Exported field
	host string   // unexported field
}

// Exported interface
type Reader interface {
	Read(p []byte) (n int, err error)  // Interface method
	Close() error
}

// Exported function
func NewServer(port int) *Server {
	return &Server{Port: port, host: "localhost"}
}

// unexported function
func helper() string {
	return "internal"
}

// Method on Server (exported)
func (s *Server) Start() error {
	return nil
}

// Method on Server (unexported)
func (s *Server) validateConfig() bool {
	return true
}
