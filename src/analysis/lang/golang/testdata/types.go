// Package types provides test fixtures for type reference extraction.
package types

import "net/http"

// --- User-defined types for testing type references ---

// Config holds application configuration.
type Config struct {
	Name    string     // built-in (should be filtered)
	Handler Handler    // direct type ref
	Cache   *Cache     // pointer type ref
	Items   []Item     // slice type ref
	Meta    map[string]Value // map value type ref
	Server  http.Server      // qualified type ref
}

// Cache is a simple cache implementation.
type Cache struct {
	TTL int
}

// Item represents a cacheable item.
type Item struct {
	ID   string
	Data interface{}
}

// Value is a generic value holder.
type Value struct {
	Raw interface{}
}

// Handler defines the handler interface.
type Handler interface {
	Handle(req Request) Response
}

// Request represents an incoming request.
type Request struct {
	Path   string
	Method string
}

// Response represents an outgoing response.
type Response struct {
	Status int
	Body   string
}

// --- Function parameter type references ---

// ProcessItems accepts a slice of Item (should extract Item).
func ProcessItems(items []Item) error {
	return nil
}

// ProcessConfig accepts a pointer to Config (should extract Config).
func ProcessConfig(config *Config) {
}

// ProcessDirect accepts Config directly (should extract Config).
func ProcessDirect(config Config) {
}

// ProcessQualified accepts a qualified type (should extract Request from http).
func ProcessQualified(req http.Request) {
}

// ProcessMultiple accepts multiple typed parameters.
func ProcessMultiple(items []Item, config *Config, cache Cache) {
}

// ProcessMap accepts a map with user-defined value type.
func ProcessMap(data map[string]Config) {
}

// ProcessMapKey accepts a map with user-defined key type.
func ProcessMapKey(data map[Item]string) {
}

// --- Function return type references ---

// NewConfig returns a pointer to Config (should extract Config).
func NewConfig() *Config {
	return &Config{}
}

// GetCache returns Cache directly (should extract Cache).
func GetCache() Cache {
	return Cache{}
}

// LoadConfigAndError returns multiple values including user type (should extract Config, NOT error).
func LoadConfigAndError(path string) (*Config, error) {
	return nil, nil
}

// GetItems returns a slice of Item (should extract Item).
func GetItems() []Item {
	return nil
}

// GetItemsAndError returns a slice with error tuple (should extract Item, NOT error).
func GetItemsAndError() ([]Item, error) {
	return nil, nil
}

// GetQualified returns a qualified type.
func GetQualified() http.Handler {
	return nil
}

// --- Method parameter and return type references ---

// Process is a method on Config that accepts Request and returns Response.
func (c *Config) Process(req Request) Response {
	return Response{}
}

// GetHandler is a method that returns Handler interface.
func (c *Config) GetHandler() Handler {
	return c.Handler
}

// SetCache is a method that accepts Cache.
func (c *Config) SetCache(cache *Cache) {
	c.Cache = cache
}

// --- Variadic parameter types ---

// ProcessMany accepts variadic Item parameters.
func ProcessMany(items ...Item) {
}

// --- Interface method signatures ---

// Processor defines methods with typed parameters and returns.
type Processor interface {
	Process(req Request) Response
	GetConfig() *Config
	SetItems(items []Item)
}

// --- Generic types (Go 1.18+) ---

// Container is a generic type with type parameter.
type Container[T any] struct {
	Value T
}

// ProcessGeneric accepts a generic Container of Item.
func ProcessGeneric(c Container[Item]) {
}

// GetContainer returns a generic Container of Config.
func GetContainer() Container[Config] {
	return Container[Config]{}
}

// --- Selector calls for Usage edges ---

// Setup makes calls to http package methods.
func Setup() {
	http.ListenAndServe(":8080", nil)
	http.HandleFunc("/", nil)
}

// --- Channel types ---

// SendItems sends items through a channel.
func SendItems(ch chan Item) {
}

// ReceiveConfigs receives configs from a channel.
func ReceiveConfigs(ch <-chan Config) {
}

// --- Type aliases ---

// Duration is an alias for int64 (primitive).
type Duration int64

// HandlerFunc is a function type alias.
type HandlerFunc func(http.ResponseWriter, *http.Request)

// ItemSlice is an alias for a slice of Item.
type ItemSlice []Item

// ConfigPtr is an alias for pointer to Config.
type ConfigPtr *Config

// CacheMap is an alias for a map type.
type CacheMap map[string]Cache
