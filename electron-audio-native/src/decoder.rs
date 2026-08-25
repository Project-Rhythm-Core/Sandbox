use std::fs::File;

use symphonia::core::{audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint};

pub fn decode_file(path: &str) -> (Vec<f32>, u32, u16) {

    let file = File::open(path).expect("No se pude abrir el archivo");
    let  mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .expect("formato no reconocido");

    let mut format = probed.format;
    let track = format.default_track().expect("no hay track de audio");
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .expect("no se pudo crear el decoder");

    let sample_rate = track.codec_params.sample_rate.expect("sin sample rate");
    let channels = track.codec_params.channels.expect("sin info de canales").count() as u16;

    let mut all_samples = Vec::new();

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        if let Ok(decoded) = decoder.decode(&packet) {
            let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            sample_buf.copy_interleaved_ref(decoded);
            all_samples.extend_from_slice(sample_buf.samples());
        }
    }

    (all_samples, sample_rate, channels)
}