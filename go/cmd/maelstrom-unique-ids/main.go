package main

import (
	"gloomers/proc"

	"github.com/google/uuid"
	maelstrom "github.com/jepsen-io/maelstrom/demo/go"
)

func main() {
	n := maelstrom.NewNode()
	n.Handle("generate", func(msg maelstrom.Message) error {
		body := make(map[string]string)
		body["type"] = "generate_ok"
		body["id"] = uuid.New().String()

		return n.Reply(msg, body)
	})

	proc.Exit(n.Run())
}
