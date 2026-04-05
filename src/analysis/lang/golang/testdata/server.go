package server

import (
	"fmt"
	"net/http"

	log "github.com/sirupsen/logrus"
)

type Server struct {
	host string
	port int
}

func NewServer(host string, port int) *Server {
	return &Server{host: host, port: port}
}

func (s *Server) Start() error {
	addr := fmt.Sprintf("%s:%d", s.host, s.port)
	log.Infof("Starting server on %s", addr)
	return http.ListenAndServe(addr, nil)
}

func (s *Server) Stop() {
	log.Info("Stopping server")
}
