use libflate::gzip::{Decoder, Encoder};
use prost::{DecodeError, Message};
use std::io::{self, Read, Write};

use thiserror::Error;

// Compressor represents compression algorithm
pub(super) enum Compressor {
    // Plain does not apply any compression algorithm to the encoded data.
    Plain,
    // Gzip compresses the data with the gzip algorithm after encoding it.
    Gzip,
}

#[derive(Error, Debug)]
pub enum CompressorError {
    #[error("encoding format not supported: `{0}`")]
    UnsupportedEncoding(String),
}

impl TryFrom<&[u8]> for Compressor {
    type Error = CompressorError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value {
            b"gzip" => Ok(Compressor::Gzip),
            val => Err(CompressorError::UnsupportedEncoding(format!("{:?}", val))),
        }
    }
}

#[derive(Error, Debug)]
pub enum EncoderError {
    #[error("`{0}`")]
    IO(#[from] io::Error),
}

#[derive(Error, Debug)]
pub enum DecoderError {
    #[error("`{0}`")]
    Prost(#[from] DecodeError),
    #[error("`{0}`")]
    IO(#[from] io::Error),
}

pub(super) fn encode_message<M>(comp: &Compressor, msg: M) -> Result<Vec<u8>, EncoderError>
where
    M: Message,
{
    let proto_enc = msg.encode_to_vec();
    match comp {
        Compressor::Plain => Ok(proto_enc),
        Compressor::Gzip => {
            let mut encoder = Encoder::new(Vec::new())?;
            encoder.write_all(&proto_enc)?;
            Ok(encoder.finish().into_result()?)
        }
    }
}

pub(super) fn decode_message<M>(comp: &Compressor, msg: &[u8]) -> Result<M, DecoderError>
where
    M: Message + Default,
{
    match comp {
        Compressor::Plain => Ok(M::decode(msg)?),
        Compressor::Gzip => {
            let mut decoder = Decoder::new(msg)?;
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf).unwrap();
            Ok(M::decode(buf.as_slice())?)
        }
    }
}
