package greplog

import (
	"crypto/sha256"
	"fmt"
	"strings"
)

// RedactionMode defines how sensitive data is masked
type RedactionMode string

const (
	Full    RedactionMode = "Full"
	Partial RedactionMode = "Partial"
	Hash    RedactionMode = "Hash"
)

var defaultRedactedKeys = map[string]RedactionMode{
	"password": Full,
	"token":    Full,
	"secret":   Full,
	"email":    Partial,
}

func redactString(val string, mode RedactionMode) string {
	if val == "" {
		return ""
	}
	switch mode {
	case Full:
		return "[REDACTED]"
	case Partial:
		if len(val) <= 4 {
			return "[***]"
		}
		return val[:2] + "***" + val[len(val)-2:]
	case Hash:
		h := sha256.New()
		h.Write([]byte(val))
		return fmt.Sprintf("[HASH:%016x]", h.Sum(nil)[:8])
	default:
		return val
	}
}

func keyMatchesPattern(key, pattern string) bool {
	return strings.Contains(strings.ToLower(key), strings.ToLower(pattern))
}

func redactAttributes(attrs map[string]string) map[string]string {
	if attrs == nil {
		return nil
	}
	result := make(map[string]string, len(attrs))
	for k, v := range attrs {
		matched := false
		for pattern, mode := range defaultRedactedKeys {
			if keyMatchesPattern(k, pattern) {
				result[k] = redactString(v, mode)
				matched = true
				break
			}
		}
		if !matched {
			result[k] = v
		}
	}
	return result
}
