use std::{
    io::{BufRead, BufReader, BufWriter, Read, Stdin, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Data, Device, InputCallbackInfo, Stream, StreamConfig, StreamError,
};
use hound::{WavSpec, WavWriter};

use crate::music::play;

mod music;

fn cpal_to_hound_sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_int() || format.is_uint() {
        hound::SampleFormat::Int
    } else {
        hound::SampleFormat::Float
    }
}

fn record<P: AsRef<Path>>(path: P, device: &Device) {
    let config = device.default_input_config().unwrap();

    let spec = WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: config.sample_format().sample_size() as u16 * 8,
        sample_format: cpal_to_hound_sample_format(config.sample_format()),
    };
    let mut writer = WavWriter::create(path, spec).unwrap();

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], info: &InputCallbackInfo| {
                data.iter().for_each(|x| writer.write_sample(*x).unwrap())
            },
            move |err: StreamError| {
                println!("{:?}", err);
            },
            None,
        )
        .unwrap();

    println!("Recording...");
    stream.play().unwrap();
    std::thread::sleep(Duration::new(5, 0));
    stream.pause().unwrap();
}

fn stt<P: AsRef<Path>>(path: P) -> String {
    let mut child = Command::new("python")
        .arg("stt.py")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    stdin
        .write_all(format!("{}\n", path.as_ref().display()).as_bytes())
        .unwrap();

    let mut speech = String::new();
    stdout.read_line(&mut speech).unwrap();
    speech
        .chars()
        .filter(|c| c.is_alphabetic() | c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
}

fn main() {
    let host = cpal::default_host();
    let input = host.default_input_device().unwrap();
    let output = host.default_output_device().unwrap();
    let mut playing: Option<Stream> = None;

    let index = if !Path::new("music.json").exists()
        || std::env::args().nth(1).unwrap_or_default() == "reindex"
    {
        let index = music::index();
        std::fs::write("music.json", serde_json::to_vec(&index).unwrap()).unwrap();
        index
    } else {
        serde_json::from_slice(&std::fs::read("music.json").unwrap()).unwrap()
    };

    loop {
        println!("Press Enter to start recording");
        let mut a = String::new();
        std::io::stdin().read_line(&mut a).unwrap();

        record("output.wav", &input);

        let speech = stt("output.wav");
        let speech = speech.trim();
        println!("{}", speech);

        if speech.split(' ').next().unwrap() == "play" {
            if speech == "play" {
                if let Some(playing) = &playing {
                    playing.play().unwrap();
                }
            } else {
                playing = Some(play(
                    speech.split(' ').skip(1).collect::<Vec<&str>>(),
                    &output,
                    &index,
                ));
            }
        }

        if speech == "pause" {
            if let Some(playing) = &playing {
                playing.pause().unwrap();
            }
        }
    }
}
