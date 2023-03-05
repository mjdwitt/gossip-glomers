package state

import "log"

type event interface {
	// event is just a marker interface
	event()
}

// State appends values to a store via a serialized interface
type State struct {
	store   []int32
	input   chan event
	closing chan struct{}
	closed  chan struct{}
}

// New returns a State
func New() *State {
	s := &State{
		store:   make([]int32, 0),
		input:   make(chan event),
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

// Append adds a value to the store
func (s *State) Append(n int32) {
	s.input <- &appendEvent{n}
}

// Read returns all stored values
func (s *State) Read() []int32 {
	output := make(chan []int32)
	s.input <- &readEvent{output}
	return <-output
}

type appendEvent struct {
	value int32
}

func (a *appendEvent) event() {}

type readEvent struct {
	output chan<- []int32
}

func (r *readEvent) event() {}

func (s *State) loop() {
	for {
		select {
		case <-s.closing:
			close(s.closed)
			return
		case ev := <-s.input:
			switch ev.(type) {
			case *appendEvent:
				s.store = append(s.store, ev.(*appendEvent).value)
			case *readEvent:
				read := make([]int32, len(s.store))
				copy(read, s.store)
				ev.(*readEvent).output <- read
			default:
				log.Panicf("invalid event %v", ev)
			}
		}
	}
}
