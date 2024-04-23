use memchr::memchr;
use memmap2::{Advice, Mmap, MmapOptions};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/known_cities.rs"));

pub struct ProcessedStation {
    pub name: Vec<u8>,
    pub min: i16,
    pub avg_tmp: i64,
    pub avg_count: usize,
    pub max: i16,
}
const NONE_PROCESSED_STATION: Option<ProcessedStation> = None;

pub fn split_file(num_threads: usize, mmap: &Mmap) -> Vec<usize> {
    let mut poses = vec![0];
    for i in 1..num_threads {
        let start = mmap.len() / num_threads * i;
        let newline = memchr(b'\n', &mmap[start..]).expect("Failed to find newline");
        let pos = start + newline + 1;
        poses.push(pos);
    }
    mmap.advise(Advice::Sequential).unwrap();

    poses
}

pub type Name = Vec<u8>;

fn parse_fixed_point(num: &[u8]) -> i16 {
    let mut pos = 0;
    let neg = num[0] == b'-';
    if neg {
        pos += 1;
    }
    let mut r = 0i16;

    r += (num[pos] - b'0') as i16;
    pos += 1;

    if num[pos] != b'.' {
        r = r * 10 + (num[pos] - b'0') as i16;
        pos += 1;
    }

    debug_assert_eq!(num[pos], b'.');
    pos += 1;
    r = r * 10 + (num[pos] - b'0') as i16;

    if neg {
        -r
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(parse_fixed_point(b"-25.3"), -253);
        assert_eq!(parse_fixed_point(b"0.0"), 0);
        assert_eq!(parse_fixed_point(b"1.0"), 10);
        assert_eq!(parse_fixed_point(b"1.5"), 15);
        assert_eq!(parse_fixed_point(b"-25.3"), -253);
    }
}

pub fn thread(
    data: Arc<Mmap>,
    start_idx: usize,
    end_idx: usize,
) -> [Option<ProcessedStation>; KNOWN_CITIES.len()] {
    let mut stations: [Option<ProcessedStation>; KNOWN_CITIES.len()] =
        [NONE_PROCESSED_STATION; KNOWN_CITIES.len()];

    let data = &data[start_idx..end_idx];

    let mut last_pos = 0;
    for next_pos in memchr::memchr_iter(b'\n', data) {
        let line = &data[last_pos..next_pos];
        last_pos = next_pos + 1;
        if line.is_empty() {
            continue;
        }

        // `City of San Marino;30.0`
        let semi_pos = memchr(b';', line).unwrap();
        let name = &line[..semi_pos];
        let temp_str = &line[semi_pos + 1..];

        let temp = parse_fixed_point(temp_str);

        let hash = KNOWN_CITIES_HASHER.get(name).unwrap_or_else(|| {
            panic!("Station {name:?} must be hash-able, as we know all stations in advance")
        });

        match unsafe { stations.get_unchecked_mut(*hash) } {
            Some(station) => {
                if temp < station.min {
                    station.min = temp;
                }
                if temp > station.max {
                    station.max = temp;
                }

                station.avg_tmp += temp as i64;
                station.avg_count += 1;
            }
            None => {
                unsafe {
                    *stations.get_unchecked_mut(*hash) = Some(ProcessedStation {
                        name: name.to_owned(),
                        min: temp,
                        avg_tmp: temp as i64,
                        avg_count: 1,
                        max: temp,
                    })
                };
            }
        }
    }

    stations
}

fn merge_stations(
    thread_data: Vec<[Option<ProcessedStation>; KNOWN_CITIES.len()]>,
) -> Vec<ProcessedStation> {
    let mut result: [Option<ProcessedStation>; KNOWN_CITIES.len()] =
        [NONE_PROCESSED_STATION; KNOWN_CITIES.len()];

    for thread_stations in thread_data.into_iter() {
        for (station_hash, station) in thread_stations.into_iter().enumerate() {
            if let Some(station) = station {
                match &mut result[station_hash] {
                    Some(s) => {
                        if station.min < s.min {
                            s.min = station.min;
                        }
                        if station.max > s.max {
                            s.max = station.max;
                        }

                        s.avg_tmp += station.avg_tmp;
                        s.avg_count += station.avg_count;
                    }
                    None => {
                        result[station_hash] = Some(station);
                    }
                }
            }
        }
    }

    result
        .into_iter()
        .map(|s| s.expect("All stations should have at least one measurement"))
        .collect()
}

pub fn solution(input_path: &Path) -> Vec<ProcessedStation> {
    let file = File::open(input_path).unwrap();
    let mmap = unsafe { MmapOptions::new().map(&file).unwrap() };
    let data: Arc<Mmap> = Arc::new(mmap);

    let num_threads = num_cpus::get();
    let poses = split_file(num_threads, &data);

    let threads: Vec<_> = (0..poses.len())
        .map(|i| {
            let data = Arc::clone(&data);
            let start = poses[i];
            let end = poses.get(i + 1).cloned().unwrap_or(data.len());
            std::thread::spawn(move || thread(data, start, end))
        })
        .collect();

    let thread_data: Vec<[Option<ProcessedStation>; KNOWN_CITIES.len()]> =
        threads.into_iter().map(|t| t.join().unwrap()).collect();

    let mut stations = merge_stations(thread_data);

    stations.sort_unstable_by_key(|s| s.name.clone());

    stations
}

pub fn format_results(stations: &[ProcessedStation]) -> String {
    let mut out = String::new();
    out.push('{');
    for (i, station) in stations.iter().enumerate() {
        use std::fmt::Write;
        let min = station.min as f32 / 10.0;
        let avg = station.avg_tmp as f32 / 10.0 / station.avg_count as f32;
        let max = station.max as f32 / 10.0;
        let name = std::str::from_utf8(&station.name).unwrap();

        let _ = write!(&mut out, "{}={min:.1}/{avg:.1}/{max:.1}", name);

        if i != stations.len() - 1 {
            let _ = write!(&mut out, ", ");
        }
    }

    out.push('}');
    out
}

#[test]
fn validate() {
    use std::time::Instant;

    let input_path = "/tmp/measurements.txt";
    let expected_out_path = "/tmp/expected_out.txt";
    let expected = std::fs::read_to_string(expected_out_path).unwrap();
    let expected = expected.trim();

    let start = Instant::now();
    let out = solution(Path::new(input_path));
    let time_taken = start.elapsed();
    println!("Took: {time_taken:?}");
    let formatted = format_results(&out);
    pretty_assertions::assert_eq!(formatted, expected);
}
