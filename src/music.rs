use std::{
    collections::HashMap,
    ffi::OsStr,
    io::Cursor,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, OutputCallbackInfo, SampleRate, Stream, StreamError,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use symphonia::core::{
    audio::AudioBufferRef,
    codecs::{CodecRegistry, CODEC_TYPE_NULL},
    io::MediaSourceStream,
    meta::StandardTagKey,
    probe::Hint,
};
use walkdir::WalkDir;

fn get_title<P: AsRef<Path>>(path: P) -> Option<String> {
    audiotags::Tag::new()
        .read_from_path(path)
        .unwrap()
        .title()
        .map(|x| x.to_owned())
}

fn fuzzy(target: &str, query: &str) -> i64 {
    if target.trim() == query.trim() {
        return i64::MAX;
    }

    let mut score = 0;

    for word in target.split(' ') {
        if query.split(' ').find(|w| w == &word).is_some() {
            score += 10;
        }
    }

    score
}

fn _play<P: AsRef<Path>>(path: P, device: &Device) -> Stream {
    let file = std::fs::File::open(path).unwrap();
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let meta_opts = Default::default();
    let fmt_opts = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &fmt_opts, &meta_opts)
        .unwrap();
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .unwrap();

    let dec_opts = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .unwrap();

    let sample_rate = track.codec_params.sample_rate.unwrap();
    let n_frames = track.codec_params.n_frames.unwrap();
    println!("{n_frames}");

    let mut ptr = 0;
    let mut buffer = Vec::with_capacity(n_frames as usize);

    let start = Instant::now();
    loop {
        let Ok(packet) = format.next_packet() else {
            break;
        };

        let Ok(decoded) = decoder.decode(&packet) else {
            break;
        };
        match decoded {
            AudioBufferRef::F32(b) => {
                //println!("{}", b.planes().planes().len());
                buffer.extend_from_slice(b.planes().planes()[0]);
            }
            _ => panic!(),
        }
    }
    println!("Decoding took {}", (Instant::now() - start).as_secs_f32());
    println!("{n_frames} {}", buffer.len());

    let mut config = device.default_output_config().unwrap().config();
    config.sample_rate.0 = sample_rate;
    config.channels = 1;

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &OutputCallbackInfo| {
                for x in data {
                    *x = buffer.get(ptr).copied().unwrap_or(0.0);
                    ptr += 1;
                }
            },
            move |_: StreamError| {},
            None,
        )
        .unwrap();
    stream.play().unwrap();
    stream
}

const EXTENSIONS: [&str; 4] = ["m4a", "m4p", "m4v", "mp3"];

pub fn index() -> HashMap<String, PathBuf> {
    let entries = WalkDir::new("/mnt/3TB/Jack/music")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .and_then(OsStr::to_str)
                .map(|ext| EXTENSIONS.contains(&ext))
                .unwrap_or_default()
        })
        .collect::<Vec<PathBuf>>();

    let length = entries.len();

    let index = entries
        .into_iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            let Some(title) = get_title(&entry) else {
                return None;
            };
            let title = title
                .chars()
                .filter(|c| c.is_whitespace() | c.is_alphabetic())
                .flat_map(|c| c.to_lowercase())
                .collect();
            println!("{i}/{length} - {}:{}", entry.display(), title);
            Some((title, entry))
        })
        .collect::<HashMap<String, PathBuf>>();

    println!("{length}/{length}");

    index
}

pub fn play(words: Vec<&str>, device: &Device, index: &HashMap<String, PathBuf>) -> Stream {
    let title = words.join(" ");
    println!("Looking for: {title}");

    let title = index
        .keys()
        //.inspect(|t| println!("{}", fuzzy(&title, t)))
        .max_by_key(|t| fuzzy(&title, t))
        .unwrap();
    println!("Playing: {} at {}", title, index[title].display());
    _play(&index[title], device)
}
