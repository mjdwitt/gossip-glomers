package topology

// Request contains the current node topology
type Request struct{}

// Type of the request
func (r *Request) Type() string {
	return "topology"
}

// Response is empty
type Response struct{}

// MarshalJSON on a Response always the same json object.
func (r *Response) MarshalJSON() ([]byte, error) {
	return []byte(`{"type":"topology_ok"}`), nil
}
