package broadcast

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestResponse(t *testing.T) {
	t.Run("marshals to constant json broadcast_ok", func(t *testing.T) {
		r := &Response{}
		json, err := r.MarshalJSON()
		assert.Equal(t, string(json), `{"type":"broadcast_ok"}`)
		assert.NoError(t, err)
	})
}
