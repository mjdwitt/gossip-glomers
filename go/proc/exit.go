package proc

import (
	"log"
	"os"
)

// Exit exits the program, logging any given error with `log.Fatal` if non-nil.
func Exit(err error) {
	if err != nil {
		log.Fatal(err)
	} else {
		os.Exit(0)
	}
}
