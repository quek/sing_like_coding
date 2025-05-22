use std::sync::{Arc, Mutex};

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig, SupportedStreamConfig};

pub struct Device {
    device: cpal::Device,
    sample_format: SampleFormat,
    config: StreamConfig,
    pub supported_stream_config: SupportedStreamConfig,
    stream: Option<Stream>,
    pub frames_per_buffer: Arc<Mutex<usize>>,
}

impl Device {
    // pub fn get_frames_per_buffer(&self) -> usize {
    //     *self.frames_per_buffer.lock().unwrap()
    // }

    pub fn open_default() -> Result<Device> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");
        println!("{:?}", device.name());
        let mut supported_configs_range = device
            .supported_output_configs()
            .expect("error while querying configs");
        let supported_stream_config = supported_configs_range
            .next()
            .expect("no supported config?!")
            .with_max_sample_rate();
        println!("{:?}", supported_stream_config);
        let sample_format = supported_stream_config.sample_format();
        let config: StreamConfig = supported_stream_config.clone().into();
        log::info!("{:?}", &config);

        Ok(Device {
            device,
            sample_format,
            config,
            supported_stream_config,
            stream: None,
            frames_per_buffer: Arc::new(Mutex::new(512)),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);

        let mut sample_clock = 0f32;
        let sample_rate = self.config.sample_rate.0 as f32;
        let channels = self.config.channels as usize;
        let frames_per_buffer = self.frames_per_buffer.clone();

        let stream = match self.sample_format {
            SampleFormat::F32 => self.device.build_output_stream(
                &self.config,
                move |output: &mut [f32], _| {
                    {
                        let mut x = frames_per_buffer.lock().unwrap();
                        *x = output.len() / channels;
                    }
                    for frame in output.chunks_mut(channels) {
                        sample_clock = (sample_clock + 1.0) % sample_rate;
                        let value =
                            (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin();
                        for sample in frame.iter_mut() {
                            *sample = value;
                        }
                    }
                },
                err_fn,
                None,
            ),
            SampleFormat::U8 => self.device.build_output_stream(
                &self.config,
                move |output: &mut [f32], _| {
                    {
                        let mut x = frames_per_buffer.lock().unwrap();
                        *x = output.len() / channels;
                        log::debug!("frames_per_buffer {}", *x);
                    }
                    for frame in output.chunks_mut(channels) {
                        sample_clock = (sample_clock + 1.0) % sample_rate;
                        let value =
                            (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin();
                        for sample in frame.iter_mut() {
                            *sample = value;
                        }
                    }
                },
                err_fn,
                None,
            ),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
        .unwrap();

        stream.play().unwrap();
        self.stream = Some(stream);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stream = None;
        Ok(())
    }
}

#[allow(dead_code)]
fn write_silence<T: Sample>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
    for sample in data.iter_mut() {
        *sample = Sample::EQUILIBRIUM;
    }
}
