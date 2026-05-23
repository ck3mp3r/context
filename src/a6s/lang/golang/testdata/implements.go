package test

// test_implements_edge_simple: MyGreeter implements Greeter
type Greeter interface {
	Greet()
}

type MyGreeter struct{}

func (m MyGreeter) Greet() {}

// test_implements_edge_no_match: BadSender has Receive() but Sender wants Send()
type Sender interface {
	Send()
}

type BadSender struct{}

func (b BadSender) Receive() {}

// test_implements_edge_partial_match: MyRCX only has ReadX(), not CloseX()
type ReadCloserX interface {
	ReadX()
	CloseX()
}

type MyRCX struct{}

func (m MyRCX) ReadX() {}

// test_implements_edge_multiple_interfaces: PingPonger satisfies Pinger and Ponger
type Pinger interface {
	Ping()
}

type Ponger interface {
	Pong()
}

type PingPonger struct{}

func (pp PingPonger) Ping() {}
func (pp PingPonger) Pong() {}

// test_implements_edge_pointer_receiver: *MyCloser satisfies Closer
type Closer interface {
	Close()
}

type MyCloser struct{}

func (m *MyCloser) Close() {}
