package state

// State appends values to a store via a serialized interface
type State struct {
	store   []int32
	input   chan int32
	closing chan struct{}
	closed  chan struct{}
}

// NewState returns a State
func NewState() *State {
	s := &State{
		store:   make([]int32, 0),
		input:   make(chan int32),
		closing: make(chan struct{}),
		closed:  make(chan struct{}),
	}
	go s.loop()
	return s
}

// Close shuts down a State's serialization channels
func (s *State) Close() {
	close(s.closing)
	<-s.closed
	close(s.input)
}

func (s *State) loop() {
	for {
		select {
		case <-s.closing:
			close(s.closed)
			return
		case n := <-s.input:
			s.store = append(s.store, n)
		}
	}
}

// Append adds a value to the store
func (s *State) Append(n int32) {
	s.input <- n
}

// Read returns all stored values
func (s *State) Read() []int32 {
	read := make([]int32, len(s.store))
	copy(read, s.store)
	return read
}
