package network

import (
	"context"
	"log"

	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
	"golang.org/x/sync/errgroup"
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
	// SyncRPC sends a synchronous RPC request. Returns the response message. RPC
	// errors in the message body are converted to *RPCError and are returned.
	SyncRPC(ctx context.Context, dest string, body any) (maelstrom.Message, error)
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

	for {
		res, err := n.node.SyncRPC(context.Background(), node, body)
		switch {
		case err == nil:
			return nil
		case res.RPCError() != nil:
			log.Printf("RPC failed: %v", err)
			continue
		default:
			return err
		}
	}
}

// MessageAll sends a message to every node in the network.
func (n *Network) MessageAll(body any) error {
	group := &errgroup.Group{}
	for _, node := range n.node.NodeIDs() {
		if node == n.node.ID() {
			continue
		}

		node := node
		group.Go(func() error { return n.MessageNode(node, body) })
	}

	return group.Wait()
}
