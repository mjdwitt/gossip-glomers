package read

import "encoding/json"

// Request is empty
type Request struct{}

// Type of the request
func (r *Request) Type() string {
	return "read"
}

// Response contains all the messages seen by this node
type Response struct {
	Messages []int32 `json:"messages"`
}

// MarshalJSON inserts a `type` field when marshalling a Response
func (r *Response) MarshalJSON() ([]byte, error) {
	type alias Response
	type aux struct {
		Type string `json:"type"`
		alias
	}

	return json.Marshal(&aux{Type: "read_ok", alias: (alias)(*r)})
}
