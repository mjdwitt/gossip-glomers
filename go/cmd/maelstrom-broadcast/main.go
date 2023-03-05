package main

import (
	"encoding/json"

	"gloomers/cmd/maelstrom-broadcast/broadcast"
	"gloomers/cmd/maelstrom-broadcast/read"
	"gloomers/cmd/maelstrom-broadcast/state"
	"gloomers/cmd/maelstrom-broadcast/topology"
	"gloomers/proc"

	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
)

func main() {
	s := state.NewState()
	defer s.Close()

	node := maelstrom.NewNode()

	node.Handle("broadcast", func(msg maelstrom.Message) error {
		var req broadcast.Request
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		s.Append(req.Message)
		return node.Reply(msg, &broadcast.Response{})
	})

	node.Handle("read", func(msg maelstrom.Message) error {
		var req read.Request
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		return node.Reply(msg, &read.Response{Messages: s.Read()})
	})

	node.Handle("topology", func(msg maelstrom.Message) error {
		var req topology.Request
		if err := json.Unmarshal(msg.Body, &req); err != nil {
			return err
		}

		return node.Reply(msg, &topology.Response{})
	})

	proc.Exit(node.Run())
}
