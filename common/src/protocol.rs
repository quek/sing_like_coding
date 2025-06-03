use bincode::{config, Decode, Encode};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum MainToPlugin {
    Load(String, usize),
    Quit,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum PluginToMain {
    DidLoad,
    Quit,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum AudioToPlugin {
    A,
    B,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum PluginToAudio {
    A,
    B,
}

pub fn send<T>(pipe: HANDLE, message: &T) -> anyhow::Result<()>
where
    T: Encode,
{
    let config = config::standard();
    let bytes: Vec<u8> = bincode::encode_to_vec(message, config).unwrap();
    let len_bytes = (bytes.len() as u32).to_le_bytes();

    unsafe {
        let mut written = 0;
        WriteFile(pipe, Some(&len_bytes), Some(&mut written), None)?;
        if written != 4 {
            return Err(
                std::io::Error::new(std::io::ErrorKind::Other, "failed to write length").into(),
            );
        }

        let mut total = 0;
        while total < bytes.len() {
            let chunk = &bytes[total..];
            let mut written = 0;
            WriteFile(pipe, Some(chunk), Some(&mut written), None)?;
            if written == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "failed to write chunk",
                )
                .into());
            }
            total += written as usize;
        }
    }
    Ok(())
}

pub fn receive<T>(pipe: HANDLE) -> anyhow::Result<T>
where
    T: Decode<()>,
{
    unsafe {
        let mut len_buf = [0u8; 4];
        let mut read = 0;

        // 長さ（4バイト）読み込み
        ReadFile(pipe, Some(&mut len_buf), Some(&mut read), None)?;
        if read != 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "failed to read length",
            )
            .into());
        }

        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buffer = vec![0u8; len];
        let mut total_read = 0;

        // 本文読み込み（ループで）
        while total_read < len {
            let chunk = &mut buffer[total_read..];
            let mut read = 0;
            ReadFile(pipe, Some(chunk), Some(&mut read), None)?;
            if read == 0 {
                return Err(
                    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "pipe closed").into(),
                );
            }
            total_read += read as usize;
        }

        let config = config::standard();
        let (message, _len): (T, usize) = bincode::decode_from_slice(&buffer[..], config)?;

        Ok(message)
    }
}
