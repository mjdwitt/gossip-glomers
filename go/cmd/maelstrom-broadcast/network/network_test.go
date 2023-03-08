package network

import (
	"context"
	"errors"
	"log"
	"testing"

	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
	"github.com/stretchr/testify/assert"
)

func TestNetworkSendUntilAck(t *testing.T) {
	t.Run("we can spy on sent messages", func(t *testing.T) {
		ready := make(chan struct{})
		close(ready)

		sent := make(chan message, 1)
		node := &mockNode{"n1", sent, nil}

		net := &Network{ready: ready, node: node}

		err := net.sendUntilAck("dest", "body")
		assert.NoError(t, err)

		select {
		case m := <-sent:
			assert.Equal(t, m, message{"dest", "body"})
		default:
			t.Fatal("no message sent")
		}
	})

	t.Run("resends until successful", func(t *testing.T) {
		sent := make(chan message)
		failures := make(chan error)
		node := &mockNode{"n1", sent, failures}

		net := New(node)
		net.Init([]string{"n1"})

		errs := make(chan error)
		running := make(chan struct{})
		go func() {
			close(running)
			errs <- net.sendUntilAck("dest", "body")
		}()
		<-running

		for _, failure := range []error{
			errors.New("a"),
			errors.New("b"),
			errors.New("c"),
		} {
			select {
			case failures <- failure:
				continue
			case msg := <-sent:
				t.Fatalf("sendUntilAck returned unexpectedly: %v", msg)
			case err := <-errs:
				t.Fatalf("sendUntilAck did not retry a failure: %v", err)
			}
		}

		close(failures)
		assert.Equal(t, <-sent, message{"dest", "body"})
		assert.NoError(t, <-errs)
	})
}

func TestNetworkMessageAll(t *testing.T) {
	t.Run("blocks until network initialized", func(t *testing.T) {
		sent := make(chan message)
		node := &mockNode{"n1", sent, nil}
		net := New(node)

		errs := make(chan error)
		running := make(chan struct{})
		go func() {
			close(running)
			errs <- net.MessageAll("body")
		}()
		<-running
		log.Printf("sut running")

		select {
		case msg := <-sent:
			t.Fatalf("message sent before network initialized: %v", msg)
		case err := <-errs:
			t.Fatalf("MessageAll returned before network initialized: %v", err)
		default:
		}

		net.Init([]string{"n2"})
		assert.Equal(t, <-sent, message{"n2", "body"})
		assert.NoError(t, <-errs)
	})

	t.Run("sends message to every other node", func(t *testing.T) {
		nodes := []string{"n1", "n2", "n3", "n4"}
		sent := make(chan message, len(nodes))
		node := &mockNode{"n1", sent, nil}
		net := New(node)
		net.Init(nodes)

		assert.NoError(t, net.MessageAll("body"))
		close(sent)

		actual := make([]string, 0)
		for n := range sent {
			actual = append(actual, n.dest)
		}
		assert.ElementsMatch(t, actual, []string{"n2", "n3", "n4"})
	})
}

type message struct {
	dest string
	body any
}

type mockNode struct {
	id string

	messages chan<- message
	failures <-chan error
}

// ID returns the identifier for this node. Only valid after "init" message
// has been received.
func (m *mockNode) ID() string {
	return m.id
}

// Send sends a message body to a given destination node and does not wait for
// a response.
func (m *mockNode) Send(dest string, body any) error {
	m.messages <- message{dest, body}
	return nil
}

// SyncRPC sends a synchronous RPC request. Returns the response message. RPC
// errors in the message body are converted to *RPCError and are returned.
func (m *mockNode) SyncRPC(
	ctx context.Context, dest string, body any,
) (maelstrom.Message, error) {
	if m.failures != nil {
		if err := <-m.failures; err != nil {
			return maelstrom.Message{}, err
		}
	}

	m.messages <- message{dest, body}
	return maelstrom.Message{}, nil
}
