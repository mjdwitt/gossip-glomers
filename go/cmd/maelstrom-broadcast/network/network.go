package network

import (
	"context"

	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
	"golang.org/x/sync/errgroup"
)

// Node describes the methods that Network uses to communicate with other nodes.
type Node interface {
	// ID returns the identifier for this node. Only valid after "init" message
	// has been received.
	ID() string
	// SyncRPC sends a synchronous RPC request. Returns the response message. RPC
	// errors in the message body are converted to *RPCError and are returned.
	SyncRPC(ctx context.Context, dest string, body any) (maelstrom.Message, error)
}

// Network provides methods by which a node may know of and communicate with its
// neighbors.
type Network struct {
	ready chan struct{}
	nodes []string
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
func (n *Network) Init(nodes []string) {
	n.nodes = make([]string, len(nodes))
	copy(n.nodes, nodes)
	close(n.ready)
}

// MessageNode sends marshals a message to JSON and sends it to a named node.
func (n *Network) MessageNode(node string, body any) error {
	for {
		res, err := n.node.SyncRPC(context.Background(), node, body)
		switch {
		case err == nil:
			return nil
		case res.RPCError() != nil:
			continue
		default:
			return err
		}
	}
}

// MessageAll sends a message to every node in the network.
func (n *Network) MessageAll(body any) error {
	<-n.ready

	group := &errgroup.Group{}
	for _, node := range n.nodes {
		node := node
		if node != n.node.ID() {
			group.Go(func() error { return n.MessageNode(node, body) })
		}
	}

	return group.Wait()
}
