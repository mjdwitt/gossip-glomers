package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestBroadcastResponse(t *testing.T) {
	t.Run("marshals to constant json broadcast_ok", func(t *testing.T) {
		r := &broadcastResponse{}
		json, err := r.MarshalJSON()
		assert.Equal(t, string(json), `{"type":"broadcast_ok"}`)
		assert.NoError(t, err)
	})
}

func TestReadResponse(t *testing.T) {
	t.Run("MarshalJSON adds `type` field", func(t *testing.T) {
		r := &readResponse{Messages: []int32{1, 2, 3, 4, 5}}
		json, err := r.MarshalJSON()
		assert.Equal(
			t, string(json), `{"type":"read_ok","messages":[1,2,3,4,5]}`)
		assert.NoError(t, err)
	})
}

func TestTopologyResponse(t *testing.T) {
	t.Run("marshals to constant json topology_ok", func(t *testing.T) {
		r := &topologyResponse{}
		json, err := r.MarshalJSON()
		assert.Equal(t, string(json), `{"type":"topology_ok"}`)
		assert.NoError(t, err)
	})
}
