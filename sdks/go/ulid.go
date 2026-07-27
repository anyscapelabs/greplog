package greplog

import (
	"crypto/rand"
	"time"

	"github.com/oklog/ulid/v2"
)

func generateULID() string {
	entropy := ulid.Monotonic(rand.Reader, 0)
	return ulid.MustNew(ulid.Timestamp(time.Now()), entropy).String()
}
