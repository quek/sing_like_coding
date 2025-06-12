use std::sync::{Arc, Mutex};

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig};

use crate::singer::Singer;

pub struct Device {
    device: cpal::Device,
    sample_format: SampleFormat,
    config: StreamConfig,
    stream: Option<Stream>,
    singer: Arc<Mutex<Singer>>,
}

impl Device {
    pub fn open_default(singer: Arc<Mutex<Singer>>) -> Result<Device> {
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
            stream: None,
            singer,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);

        {
            let sample_rate = self.config.sample_rate.0 as f64;
            self.singer.lock().unwrap().song.sample_rate = sample_rate;
        }

        let channels = self.config.channels as usize;
        let singer = self.singer.clone();
        let stream = match self.sample_format {
            SampleFormat::U8 => self.device.build_output_stream(
                &self.config,
                move |output: &mut [f32], _| {
                    //log::debug!("callback output.len {}", output.len());
                    singer.lock().unwrap().process(output, channels).unwrap();
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

    pub fn start_p(&self) -> bool {
        self.stream.is_some()
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
