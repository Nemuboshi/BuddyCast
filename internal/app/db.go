package app

import (
	"encoding/binary"
	"fmt"
	"math"
)

// headerField is one field of the two fixed UDCast DB headers, read little-endian in order.
type headerField struct {
	name string
	read func(*[]byte) any
}

var headerFields = []headerField{
	{"SampleRate", func(p *[]byte) any { return int64(readI32(p)) }},
	{"BitPerSample", func(p *[]byte) any { return int64(readI16(p)) }},
	{"FrameShift", func(p *[]byte) any { return int64(readI16(p)) }},
	{"FrameSize", func(p *[]byte) any { return int64(readI16(p)) }},
	{"MFCCDimension", func(p *[]byte) any { return int64(readI16(p)) }},
	{"MFCCCorrelationDimensionStart", func(p *[]byte) any { return int64(readI16(p)) }},
	{"MFCCCorrelationDimensionEnd", func(p *[]byte) any { return int64(readI16(p)) }},
	{"FFTDimension", func(p *[]byte) any { return int64(readI16(p)) }},
	{"EvaluationThreshold", func(p *[]byte) any { return float64(readF32(p)) }},
	{"EvaluationCoefficient", func(p *[]byte) any { return floatNumber(readF32(p)) }},
	{"CorrelationThreshold", func(p *[]byte) any { return float64(readF32(p)) }},
	{"ComparisonInterval", func(p *[]byte) any { return int64(readI16(p)) }},
	{"DeletedSameData", func(p *[]byte) any { v := int64(int8((*p)[0])); *p = (*p)[1:]; return v }},
	{"DataLength", func(p *[]byte) any { return readI64(p) }},
	{"MovieStartTime", func(p *[]byte) any { return readI64(p) }},
}

func parseDB(data []byte, path string) (map[string]any, error) {
	if len(data) < 102 {
		return nil, fmt.Errorf("database file is too small: %s", path)
	}
	rest := data
	version := readF32(&rest)
	mainHeader := readHeader(&rest)
	subHeader := readHeader(&rest)
	offset := len(data) - len(rest)
	mainLen := mainHeader["DataLength"].(int64)
	subLen := subHeader["DataLength"].(int64)
	endMain := min(offset+int(mainLen), len(data))
	endSub := min(endMain+int(subLen), len(data))
	return map[string]any{
		"path": path, "file_size": len(data), "dbVersion": floatNumber(version),
		"header_offset_after_two_headers":      offset,
		"mainHeader":                           mainHeader,
		"subHeader":                            subHeader,
		"payload_length_expected_from_headers": mainLen + subLen,
		"payload_length_actual":                len(data) - offset,
		"size_matches_header":                  offset+int(mainLen+subLen) == len(data),
		"mainData":                             byteStats(data[offset:endMain]),
		"subData":                              byteStats(data[endMain:endSub]),
	}, nil
}

func readHeader(rest *[]byte) map[string]any {
	header := make(map[string]any, len(headerFields))
	for _, field := range headerFields {
		header[field.name] = field.read(rest)
	}
	return header
}

func readI16(p *[]byte) int16 { v := int16(binary.LittleEndian.Uint16(*p)); *p = (*p)[2:]; return v }
func readI32(p *[]byte) int32 { v := int32(binary.LittleEndian.Uint32(*p)); *p = (*p)[4:]; return v }
func readI64(p *[]byte) int64 { v := int64(binary.LittleEndian.Uint64(*p)); *p = (*p)[8:]; return v }
func readF32(p *[]byte) float32 {
	v := math.Float32frombits(binary.LittleEndian.Uint32(*p))
	*p = (*p)[4:]
	return v
}
func floatNumber(v float32) any {
	if v == float32(int64(v)) {
		return int64(v)
	}
	return float64(v)
}
func byteStats(data []byte) map[string]any {
	if len(data) == 0 {
		return map[string]any{"length": 0}
	}
	n := min(32, len(data))
	signed := make([]int16, n)
	for i := range n {
		signed[i] = int16(int8(data[i]))
	}
	return map[string]any{
		"length":                      len(data),
		"first_16_hex":                fmt.Sprintf("%x", data[:min(16, len(data))]),
		"first_32_signed_byte_values": signed,
	}
}
