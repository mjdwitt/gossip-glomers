package main

import (
	"encoding/json"

	"glomers/cmd/maelstrom-broadcast/network"
	"glomers/cmd/maelstrom-broadcast/state"
	"glomers/proc"

	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
)

func main() {
	state := state.New()
	defer state.Close()

	node := maelstrom.NewNode()
	net := network.New(node)

	node.Handle("init", func(maelstrom.Message) error {
		net.Init(node.NodeIDs())
		return nil
	})

	node.Handle("broadcast", func(msg maelstrom.Message) error {
		var req broadcastRequest
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		state.Append(req.Message)
		go net.MessageAll(&relayRequest{Message: req.Message})
		return node.Reply(msg, &broadcastResponse{})
	})

	node.Handle("relay", func(msg maelstrom.Message) error {
		var req relayRequest
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		state.Append(req.Message)
		return node.Reply(msg, &relayResponse{})
	})

	node.Handle("relay_ok", func(maelstrom.Message) error { return nil })

	node.Handle("read", func(msg maelstrom.Message) error {
		var req readRequest
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		return node.Reply(msg, &readResponse{Messages: state.Read()})
	})

	node.Handle("topology", func(msg maelstrom.Message) error {
		var req topologyRequest
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		return node.Reply(msg, &topologyResponse{})
	})

	proc.Exit(node.Run())
}

type broadcastRequest struct {
	Message int32 `json:"message"`
}

type broadcastResponse struct{}

func (r *broadcastResponse) MarshalJSON() ([]byte, error) {
	return []byte(`{"type":"broadcast_ok"}`), nil
}

type relayRequest struct {
	Message int32 `json:"message"`
}

func (r *relayRequest) MarshalJSON() ([]byte, error) {
	type alias relayRequest
	type aux struct {
		Type string `json:"type"`
		alias
	}

	return json.Marshal(&aux{Type: "relay", alias: (alias)(*r)})
}

type relayResponse struct{}

func (r *relayResponse) MarshalJSON() ([]byte, error) {
	return []byte(`{"type":"relay_ok"}`), nil
}

type readRequest struct{}

type readResponse struct {
	Messages []int32 `json:"messages"`
}

func (r *readResponse) MarshalJSON() ([]byte, error) {
	type alias readResponse
	type aux struct {
		Type string `json:"type"`
		alias
	}

	return json.Marshal(&aux{Type: "read_ok", alias: (alias)(*r)})
}

type topologyRequest struct {
	Topology map[string][]string `json:"topology"`
}

type topologyResponse struct{}

func (r *topologyResponse) MarshalJSON() ([]byte, error) {
	return []byte(`{"type":"topology_ok"}`), nil
}
