package greplog

import (
	"os"
	"regexp"
	"strings"
)

type Framework int

const (
	FrameworkNone Framework = iota
	FrameworkGin
	FrameworkEcho
	FrameworkFiber
)

var detectedFramework = FrameworkNone

func detectFramework() Framework {
	data, err := os.ReadFile("go.mod")
	if err != nil {
		return FrameworkNone
	}
	content := string(data)

	if strings.Contains(content, "gin-gonic/gin") {
		return FrameworkGin
	}
	if strings.Contains(content, "labstack/echo") {
		return FrameworkEcho
	}
	if strings.Contains(content, "gofiber/fiber") {
		return FrameworkFiber
	}
	return FrameworkNone
}

func detectServiceName() string {
	data, err := os.ReadFile("go.mod")
	if err != nil {
		return "unknown-service"
	}
	re := regexp.MustCompile(`^module\s+(\S+)`)
	matches := re.FindStringSubmatch(string(data))
	if len(matches) < 2 {
		return "unknown-service"
	}
	return matches[1]
}

func detectInstanceID() string {
	return generateULID()
}
