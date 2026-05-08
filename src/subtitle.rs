use std::borrow::Cow;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use quick_xml::de::from_str;
use serde::Deserialize;

use crate::error::BuddyCastError;
use crate::model::SubtitleEvent;

const ASS_HEADER: &str = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes
YCbCr Matrix: TV.709

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Noto Sans JP,54,&H00FFFFFF,&H0000FFFF,&H00000000,&H64000000,0,0,0,0,100,100,0,0,1,2,0,2,60,60,40,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;

/// Parse an OXK subtitle file from disk.
pub fn parse_oxk_file(path: &Path) -> Result<Vec<SubtitleEvent>> {
    let xml_text = fs::read_to_string(path)
        .with_context(|| format!("failed to read subtitle file: {}", path.display()))?;
    parse_oxk_text(&xml_text)
}

/// Parse OXK XML text into strongly typed subtitle events.
pub fn parse_oxk_text(xml_text: &str) -> Result<Vec<SubtitleEvent>> {
    let xml_text = xml_text.strip_prefix('\u{feff}').unwrap_or(xml_text);
    let root: Root =
        from_str(xml_text).map_err(|error| BuddyCastError::SubtitleParse(error.to_string()))?;
    Ok(root
        .sub_data
        .lines
        .into_iter()
        .map(|line| SubtitleEvent {
            start: line.start,
            end: line.end,
            alignment: line.alignment.unwrap_or_else(|| "bottom".to_string()),
            arrangement: line.arrangement.unwrap_or_default(),
            text: line.text.unwrap_or_default(),
            comment: line.comment.unwrap_or_default(),
        })
        .collect())
}

/// Render subtitle events into ASS text with the exact formatting used by the prototype.
pub fn render_ass(events: &[SubtitleEvent]) -> String {
    let mut lines = Vec::with_capacity(events.len() + 1);
    lines.push(ASS_HEADER.to_string());

    for event in events {
        let line = format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}{}",
            seconds_to_ass_time(event.start),
            seconds_to_ass_time(event.end),
            build_ass_override(&event.alignment),
            normalize_text(&event.text),
        );
        lines.push(line);
    }

    lines.join("\n")
}

/// Render subtitle events into SRT text.
pub fn render_srt(events: &[SubtitleEvent]) -> String {
    let mut blocks = Vec::with_capacity(events.len());
    for (index, event) in events.iter().enumerate() {
        blocks.push(format!(
            "{}\n{} --> {}\n{}",
            index + 1,
            seconds_to_srt_time(event.start),
            seconds_to_srt_time(event.end),
            normalize_text(&event.text),
        ));
    }

    if blocks.is_empty() {
        String::new()
    } else {
        format!("{}\n", blocks.join("\n\n"))
    }
}

/// Convert ruby markup from the source format into plain readable text.
pub fn convert_ruby(text: &str) -> String {
    let mut remaining = text;
    let mut output = String::new();

    loop {
        let Some(r1_index) = remaining.find(r"{\r1}") else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..r1_index]);
        let after_r1 = &remaining[r1_index + 5..];

        let Some(r2_index) = after_r1.find(r"{\r2}") else {
            output.push_str(&remaining[r1_index..]);
            break;
        };
        let kanji = &after_r1[..r2_index];
        let after_r2 = &after_r1[r2_index + 5..];

        let Some(r0_index) = after_r2.find(r"{\r0}") else {
            output.push_str(&remaining[r1_index..]);
            break;
        };
        let ruby = &after_r2[..r0_index];
        output.push_str(kanji);
        output.push('(');
        output.push_str(ruby);
        output.push(')');
        remaining = &after_r2[r0_index + 5..];
    }

    output
}

/// Normalize optional subtitle text into a printable string.
pub fn normalize_text(text: &str) -> Cow<'_, str> {
    Cow::Owned(convert_ruby(text))
}

/// Convert seconds into ASS `H:MM:SS.CC` format.
pub fn seconds_to_ass_time(seconds: f64) -> String {
    let mut total_centiseconds = (seconds * 100.0).round() as u64;
    let hours = total_centiseconds / 360_000;
    total_centiseconds %= 360_000;
    let minutes = total_centiseconds / 6_000;
    total_centiseconds %= 6_000;
    let secs = total_centiseconds / 100;
    let centis = total_centiseconds % 100;
    format!("{hours}:{minutes:02}:{secs:02}.{centis:02}")
}

/// Convert seconds into SRT `HH:MM:SS,mmm` format.
pub fn seconds_to_srt_time(seconds: f64) -> String {
    let mut total_milliseconds = (seconds * 1000.0).round() as u64;
    let hours = total_milliseconds / 3_600_000;
    total_milliseconds %= 3_600_000;
    let minutes = total_milliseconds / 60_000;
    total_milliseconds %= 60_000;
    let secs = total_milliseconds / 1000;
    let millis = total_milliseconds % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02},{millis:03}")
}

fn build_ass_override(alignment: &str) -> &'static str {
    match alignment {
        "bottom" => "{\\an2}",
        "bottom_left" => "{\\an1}",
        "bottom_right" => "{\\an3}",
        _ => "",
    }
}

#[derive(Debug, Deserialize)]
struct Root {
    #[serde(rename = "SubData")]
    sub_data: SubData,
}

#[derive(Debug, Deserialize)]
struct SubData {
    #[serde(rename = "SubLineData", default)]
    lines: Vec<SubLineData>,
}

#[derive(Debug, Deserialize)]
struct SubLineData {
    #[serde(rename = "Start", default)]
    start: f64,
    #[serde(rename = "End", default)]
    end: f64,
    #[serde(rename = "Alignment")]
    alignment: Option<String>,
    #[serde(rename = "Arrangement")]
    arrangement: Option<String>,
    #[serde(rename = "Text")]
    text: Option<String>,
    #[serde(rename = "Comment")]
    comment: Option<String>,
}
