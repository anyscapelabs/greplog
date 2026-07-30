package greplog

import (
	"google.golang.org/protobuf/proto"

	core "github.com/greplog/greplog-go/core/v1"
)

func encodeIngestBatch(serviceName, instanceID string, batchSeq int64, events []*core.LogEvent, spans []*core.Span) ([]byte, error) {
	batch := &core.IngestBatch{
		ServiceName: serviceName,
		InstanceId:  instanceID,
		BatchSeq:    batchSeq,
		Logs:        events,
		Spans:       spans,
	}
	return proto.Marshal(batch)
}
