use std::thread::sleep;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, StreamConfig};

fn main() {
    println!("Hello, world!");
    let host = cpal::default_host();
    // for device in host.output_devices().unwrap() {
    //     println!("{:?}", device.name());
    // }
    let device = host
        .output_devices()
        .unwrap()
        .find(|x| x.name().unwrap() == "Analog Out (1+2) (Prism Sound WDM Audio Device)")
        .unwrap();
    println!("{:?}", device.name());
    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();
    let sample_format = supported_config.sample_format();
    println!("{:?}", supported_config);
    let config: StreamConfig = supported_config.into();

    let channels = config.channels as usize;
    let sample_rate = config.sample_rate.0 as f32;
    let mut sample_clock = 0f32;
    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0) % sample_rate;
        (sample_clock * 440.0 * 2.0 * 3.141592 / sample_rate).sin()
    };

    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);

    let stream = match sample_format {
        SampleFormat::F32 => {
            device.build_output_stream(&config, write_silence::<f32>, err_fn, None)
        }
        SampleFormat::U8 => device.build_output_stream(
            &config,
            move |data: &mut [u8], _| write_data(data, channels, &mut next_value),
            err_fn,
            None,
        ),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }
    .unwrap();

    stream.play().unwrap();
    sleep(Duration::from_millis(1000));
}

fn write_silence<T: Sample>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
    for sample in data.iter_mut() {
        *sample = Sample::EQUILIBRIUM;
    }
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        let sample = next_sample();
        let value = T::from_sample::<f32>(sample);
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
