package read

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestResponse(t *testing.T) {
	t.Run("MarshalJSON adds `type` field", func(t *testing.T) {
		r := &Response{Messages: []int32{1, 2, 3, 4, 5}}
		json, err := r.MarshalJSON()
		assert.Equal(
			t, string(json), `{"type":"read_ok","messages":[1,2,3,4,5]}`)
		assert.NoError(t, err)
	})
}
