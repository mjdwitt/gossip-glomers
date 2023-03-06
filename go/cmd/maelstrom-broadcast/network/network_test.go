package network

import (
	"testing"

	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
	"github.com/stretchr/testify/assert"
)

func TestNetworkMessageNode(t *testing.T) {
	t.Run("we can spy on sent messages", func(t *testing.T) {
		ready := make(chan struct{})
		close(ready)

		sent := make(chan message, 1)
		node := &mockNode{"n1", []string{"n1"}, sent}

		net := &Network{ready: ready, node: node}

		err := net.MessageNode("dest", "body")
		assert.NoError(t, err)

		select {
		case m := <-sent:
			assert.Equal(t, m, message{"dest", "body"})
		default:
			t.Fatal("no message sent")
		}
	})

	t.Run("blocks until network initialized", func(t *testing.T) {
		sent := make(chan message)
		node := &mockNode{"n1", []string{"n1"}, sent}
		net := New(node)

		errs := make(chan error)
		running := make(chan struct{})
		go func() {
			close(running)
			errs <- net.MessageNode("dest", "body")
		}()
		<-running

		select {
		case msg := <-sent:
			t.Fatalf("a message was sent before topology was set. sent = %v", msg)
		case err := <-errs:
			t.Fatalf("MessageNode returned before topology was set. return = %v", err)
		default:
		}

		net.Init()
		assert.Equal(t, <-sent, message{"dest", "body"})
		assert.NoError(t, <-errs)
	})
}

func TestNetworkMessageAll(t *testing.T) {
	t.Run("sends message to every other node in topology", func(t *testing.T) {
		nodes := []string{"n1", "n2", "n3", "n4"}
		sent := make(chan message, len(nodes))
		node := &mockNode{"n1", nodes, sent}
		net := New(node)
		net.Init()

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
	id   string
	ids  []string
	send chan<- message
}

// ID returns the identifier for this node. Only valid after "init" message
// has been received.
func (m *mockNode) ID() string {
	return m.id
}

// NodeIDs returns a list of all node IDs in the cluster. This list include
// the local node ID and is the same order across all nodes. Only valid after
// "init" message has been received.
func (m *mockNode) NodeIDs() []string {
	return m.ids
}

// RPC sends an async RPC request. Handler invoked when response message
// received.
func (m *mockNode) RPC(dest string, body any, handler maelstrom.HandlerFunc) error {
	panic("not implemented") // TODO: Implement
}

// Send sends a message body to a given destination node and does not wait for
// a response.
func (m *mockNode) Send(dest string, body any) error {
	m.send <- message{dest, body}
	return nil
}
