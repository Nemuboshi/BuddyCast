use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::error::BuddyCastError;

const HEADER_FIELDS: [(&str, FieldType); 15] = [
    ("SampleRate", FieldType::I32),
    ("BitPerSample", FieldType::I16),
    ("FrameShift", FieldType::I16),
    ("FrameSize", FieldType::I16),
    ("MFCCDimension", FieldType::I16),
    ("MFCCCorrelationDimensionStart", FieldType::I16),
    ("MFCCCorrelationDimensionEnd", FieldType::I16),
    ("FFTDimension", FieldType::I16),
    ("EvaluationThreshold", FieldType::F32),
    ("EvaluationCoefficient", FieldType::F32),
    ("CorrelationThreshold", FieldType::F32),
    ("ComparisonInterval", FieldType::I16),
    ("DeletedSameData", FieldType::I8),
    ("DataLength", FieldType::I64),
    ("MovieStartTime", FieldType::I64),
];

/// Parse a database file from disk into the same JSON-friendly structure as the prototype.
pub fn parse_db_file(path: &Path, display_path: Option<&str>) -> Result<Value> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read db file: {}", path.display()))?;
    parse_db_bytes(&bytes, display_path.unwrap_or(&path.display().to_string()))
}

/// Parse database bytes directly.
pub fn parse_db_bytes(data: &[u8], path: &str) -> Result<Value> {
    if data.len() < 102 {
        return Err(BuddyCastError::DbTooSmall {
            path: path.to_string(),
        }
        .into());
    }

    let mut offset = 0usize;
    let version = read_f32(data, &mut offset, "dbVersion")?;
    let main_header = read_header(data, &mut offset)?;
    let sub_header = read_header(data, &mut offset)?;

    let main_len = as_i64(&main_header, "DataLength")? as usize;
    let sub_len = as_i64(&sub_header, "DataLength")? as usize;
    let expected_size = offset + main_len + sub_len;

    let main_data = data.get(offset..offset + main_len).unwrap_or(&[]);
    let sub_data = data
        .get(offset + main_len..offset + main_len + sub_len)
        .unwrap_or(&[]);

    Ok(json!({
        "path": path,
        "file_size": data.len(),
        "dbVersion": number_value_from_f32(version),
        "header_offset_after_two_headers": offset,
        "mainHeader": Value::Object(main_header),
        "subHeader": Value::Object(sub_header),
        "payload_length_expected_from_headers": main_len + sub_len,
        "payload_length_actual": data.len().saturating_sub(offset),
        "size_matches_header": expected_size == data.len(),
        "mainData": byte_stats(main_data),
        "subData": byte_stats(sub_data),
    }))
}

fn read_header(data: &[u8], offset: &mut usize) -> Result<Map<String, Value>> {
    let mut header = Map::new();
    for (name, field_type) in HEADER_FIELDS {
        let value = read_field(data, offset, name, field_type)?;
        header.insert(name.to_string(), value);
    }
    Ok(header)
}

fn read_field(
    data: &[u8],
    offset: &mut usize,
    field: &'static str,
    field_type: FieldType,
) -> Result<Value> {
    match field_type {
        FieldType::I8 => Ok(Value::from(read_i8(data, offset, field)?)),
        FieldType::I16 => Ok(Value::from(read_i16(data, offset, field)?)),
        FieldType::I32 => Ok(Value::from(read_i32(data, offset, field)?)),
        FieldType::I64 => Ok(Value::from(read_i64(data, offset, field)?)),
        FieldType::F32 => Ok(number_value_from_f32(read_f32(data, offset, field)?)),
    }
}

fn number_value_from_f32(value: f32) -> Value {
    if value.fract() == 0.0 && value.is_finite() {
        Value::from(value as i64)
    } else {
        Value::from(value)
    }
}

fn byte_stats(blob: &[u8]) -> Value {
    if blob.is_empty() {
        return json!({ "length": 0 });
    }

    let signed_values: Vec<i16> = blob
        .iter()
        .take(32)
        .map(|byte| {
            if *byte < 128 {
                i16::from(*byte)
            } else {
                i16::from(*byte) - 256
            }
        })
        .collect();

    json!({
        "length": blob.len(),
        "first_16_hex": hex_lower(&blob[..blob.len().min(16)]),
        "first_32_signed_byte_values": signed_values,
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn as_i64(map: &Map<String, Value>, key: &str) -> Result<i64> {
    map.get(key)
        .and_then(Value::as_i64)
        .with_context(|| format!("missing integer header field: {key}"))
}

fn read_i8(data: &[u8], offset: &mut usize, field: &'static str) -> Result<i8> {
    let bytes = read_exact(data, offset, 1, field)?;
    Ok(bytes[0] as i8)
}

fn read_i16(data: &[u8], offset: &mut usize, field: &'static str) -> Result<i16> {
    let bytes = read_exact(data, offset, 2, field)?;
    Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_i32(data: &[u8], offset: &mut usize, field: &'static str) -> Result<i32> {
    let bytes = read_exact(data, offset, 4, field)?;
    Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_i64(data: &[u8], offset: &mut usize, field: &'static str) -> Result<i64> {
    let bytes = read_exact(data, offset, 8, field)?;
    Ok(i64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn read_f32(data: &[u8], offset: &mut usize, field: &'static str) -> Result<f32> {
    let bytes = read_exact(data, offset, 4, field)?;
    Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_exact<'a>(
    data: &'a [u8],
    offset: &mut usize,
    size: usize,
    field: &'static str,
) -> Result<&'a [u8]> {
    if *offset + size > data.len() {
        return Err(BuddyCastError::DbTruncated {
            field,
            offset: *offset,
        }
        .into());
    }
    let bytes = &data[*offset..*offset + size];
    *offset += size;
    Ok(bytes)
}

#[derive(Debug, Clone, Copy)]
enum FieldType {
    I8,
    I16,
    I32,
    I64,
    F32,
}
