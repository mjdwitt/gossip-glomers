package network

import (
	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
)

// Node describes the methods that Network uses to communicate with other nodes.
type Node interface {
	// ID returns the identifier for this node. Only valid after "init" message
	// has been received.
	ID() string
	// NodeIDs returns a list of all node IDs in the cluster. This list include
	// the local node ID and is the same order across all nodes. Only valid after
	// "init" message has been received.
	NodeIDs() []string
	// RPC sends an async RPC request. Handler invoked when response message
	// received.
	RPC(dest string, body any, handler maelstrom.HandlerFunc) error
	// Send sends a message body to a given destination node and does not wait for
	// a response.
	Send(dest string, body any) error
}

// Network provides methods by which a node may know of and communicate with its
// neighbors.
type Network struct {
	ready chan struct{}
	node  Node
}

// New returns a new Network.
func New(node Node) *Network {
	return &Network{
		ready: make(chan struct{}),
		node:  node,
	}
}

// Init brings the network up.
func (n *Network) Init() {
	close(n.ready)
}

// MessageNode sends marshals a message to JSON and sends it to a named node.
func (n *Network) MessageNode(node string, body any) error {
	<-n.ready
	return n.node.Send(node, body)
}

// MessageAll sends a message to every node in the network.
func (n *Network) MessageAll(body any) error {
	for _, node := range n.node.NodeIDs() {
		if node == n.node.ID() {
			continue
		}

		if err := n.MessageNode(node, body); err != nil {
			return err
		}
	}

	return nil
}
