// electron-audio-native/src/decoder.rs
use std::fs::File;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// PCM intercalado listo para enviar al dispositivo, sin ninguna conversion pendiente.
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl DecodedAudio {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn duration_ms(&self) -> f64 {
        (self.frames() as f64 / self.sample_rate as f64) * 1000.0
    }
}

pub fn decode_file(path: &str) -> Result<DecodedAudio, String> {
    let file = File::open(path).map_err(|e| format!("no se pudo abrir '{}': {}", path, e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // La extension ayuda al probe a acertar el formato a la primera.
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("formato no reconocido: {}", e))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("no hay track de audio")?;
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("no se pudo crear el decoder: {}", e))?;

    let sample_rate = track.codec_params.sample_rate.ok_or("sin sample rate")?;
    let channels = track.codec_params.channels.ok_or("sin info de canales")?.count() as u16;

    // Reservar de golpe evita decenas de realloc a mitad de la decodificacion.
    let mut samples = Vec::with_capacity(
        track
            .codec_params
            .n_frames
            .map(|f| f as usize * channels as usize)
            .unwrap_or(sample_rate as usize * channels as usize * 60),
    );

    // Un unico SampleBuffer reutilizado en vez de uno nuevo por paquete.
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("error leyendo el archivo: {}", e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let buf = sample_buf.get_or_insert_with(|| {
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec())
                });
                buf.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buf.samples());
            }
            // Los paquetes corruptos sueltos no deben tumbar la carga entera.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("error decodificando: {}", e)),
        }
    }

    if samples.is_empty() {
        return Err("el archivo no contiene muestras decodificables".into());
    }

    Ok(DecodedAudio { samples, sample_rate, channels })
}
