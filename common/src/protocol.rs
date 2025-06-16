use bincode::{config, Decode, Encode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{audio_buffer::AudioBuffer, plugin::param::Param};

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum MainToPlugin {
    Hwnd(isize),
    Load(usize, String, usize, bool),
    Unload(usize, usize),
    GuiOpen(usize, usize),
    Params(usize, usize),
    StateLoad(usize, usize, Vec<u8>),
    StateSave(usize, usize),
    Scan,
    Quit,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum PluginToMain {
    DidHwnd,
    DidLoad,
    DidUnload(usize, usize),
    DidGuiOpen,
    DidParams(Vec<Param>),
    DidStateLoad,
    DidStateSave(usize, usize, Vec<u8>),
    DidScan,
    Quit,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum AudioToPlugin {
    Process(AudioBuffer),
    B,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum PluginToAudio {
    Process(AudioBuffer),
    B,
}

pub async fn send<T, P>(pipe: &mut P, message: &T) -> anyhow::Result<()>
where
    T: Encode,
    P: AsyncWriteExt + Unpin,
{
    let config = config::standard();
    let bytes: Vec<u8> = bincode::encode_to_vec(message, config).unwrap();

    pipe.write_u32_le(bytes.len() as u32).await?;
    pipe.write_all(&bytes).await?;
    pipe.flush().await?;
    Ok(())
}

pub async fn receive<T, P>(pipe: &mut P) -> anyhow::Result<T>
where
    T: Decode<()>,
    P: AsyncReadExt + Unpin,
{
    let len = pipe.read_u32_le().await?;
    let mut buffer = vec![0u8; len as usize];
    pipe.read_exact(&mut buffer).await?;

    let config = config::standard();
    let (message, _len): (T, usize) = bincode::decode_from_slice(&buffer[..], config)?;

    Ok(message)
}
