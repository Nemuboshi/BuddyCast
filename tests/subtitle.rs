use buddy_cast::subtitle::{
    convert_ruby, parse_oxk_file, render_ass, seconds_to_ass_time, seconds_to_srt_time,
};

#[test]
fn oxk_to_ass_matches_expected() {
    let events = parse_oxk_file(std::path::Path::new("fixtures/subtitles_ja.oxk.decrypted"))
        .expect("subtitle fixture should parse");
    let actual = render_ass(&events);
    let expected = std::fs::read_to_string("fixtures/expected.ass").expect("fixture should exist");
    assert_eq!(actual, expected);
}

#[test]
fn ruby_conversion_matches_expected_shape() {
    assert_eq!(
        convert_ruby(r"{\r1}男{\r2}おとこ{\r0}です"),
        "男(おとこ)です"
    );
}

#[test]
fn time_formatting_matches_prototype() {
    assert_eq!(seconds_to_ass_time(1.23), "0:00:01.23");
    assert_eq!(seconds_to_srt_time(6.5), "00:00:06,500");
}
