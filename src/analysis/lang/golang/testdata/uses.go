package uses

// Constants to be referenced
const ClientName = "myclient"
const maxRetries = 3
const MaxSize = 100

// Variables to be referenced
var defaultTimeout = 30

// Function that returns a constant
func GetClientName() string {
	return ClientName
}

// Function that returns a variable
func GetTimeout() int {
	return defaultTimeout
}

// Function that uses constant in comparison
func ShouldRetry(count int) bool {
	return count < maxRetries
}

// Function that uses short var decl with const
func Setup() {
	size := MaxSize
	_ = size
}

// Function that uses constant as argument
func LogName() {
	println(ClientName)
}
