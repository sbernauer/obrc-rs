use std::{collections::HashSet, path::Path};

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input_path: &String = args.get(1).expect("Need two args: input and output file");
    let expected_out_path = args.get(2).expect("Need two args: input and output file");

    let out = obrc_rs::solution(Path::new(input_path));

    let _names: Vec<_> = out
        .iter()
        .map(|o| std::str::from_utf8(&o.name).unwrap().to_owned())
        .collect();
    let names: Vec<_> = out.iter().map(|o| o.name.to_owned()).collect();
    let min = names.iter().map(|n| n.len()).min().unwrap();
    dbg!(min);

    let three_digs: HashSet<_> = names.iter().map(|n| n[..3].to_owned()).collect();
    assert_eq!(three_digs.len(), names.len());

    check(out, expected_out_path);
}

fn check(out: Vec<obrc_rs::ProcessedStation>, expected_out_path: &str) {
    let formatted = obrc_rs::format_results(&out);

    let expected = std::fs::read_to_string(expected_out_path).unwrap();
    let expected = expected.trim();
    pretty_assertions::assert_eq!(formatted, expected);
}
