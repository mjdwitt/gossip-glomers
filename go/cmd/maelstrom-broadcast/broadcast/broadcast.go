package broadcast

// Request type for a `broadcast` rpc
type Request struct {
	Message int32 `json:"message"`
}

// Type of the request
func (r *Request) Type() string {
	return "broadcast"
}

// Response type for a `broadcast` rpc
type Response struct{}

// MarshalJSON on a Response always the same json object.
func (r *Response) MarshalJSON() ([]byte, error) {
	return []byte(`{"type":"broadcast_ok"}`), nil
}
