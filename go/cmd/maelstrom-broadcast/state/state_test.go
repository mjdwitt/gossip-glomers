package state

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"golang.org/x/sync/errgroup"
)

func TestState(t *testing.T) {
	t.Run("empty store can be read", func(t *testing.T) {
		assert.Equal(t, NewState().Read(), []int32{})
	})

	t.Run("one append can be read", func(t *testing.T) {
		s := NewState()
		defer s.Close()
		s.Append(1)
		assert.Equal(t, s.Read(), []int32{1})
	})

	t.Run("many serial appends can be read", func(t *testing.T) {
		s := NewState()
		defer s.Close()

		for _, n := range []int32{1, 2, 3, 4, 5} {
			s.Append(n)
		}

		assert.Equal(t, s.Read(), []int32{1, 2, 3, 4, 5})
	})

	t.Run("append after close fails", func(t *testing.T) {
		s := NewState()
		s.Close()
		assert.Panics(t, func() { s.Append(1) })
	})

	t.Run("concurrent appends can be read", func(t *testing.T) {
		s := NewState()
		defer s.Close()

		limit := int32(4_000)
		values := make([]int32, limit)
		for i := int32(0); i < limit; i++ {
			values[i] = i
		}

		group := new(errgroup.Group)
		for n := range mapset(values) {
			n := n
			group.Go(func() error {
				s.Append(n)
				return nil
			})
		}
		group.Wait()

		assert.ElementsMatch(t, s.Read(), values)
	})
}

func mapset(slice []int32) map[int32]any {
	set := make(map[int32]any, len(slice))
	for _, n := range slice {
		set[n] = nil
	}
	return set
}
