use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::BackendSpecificError;
use std::sync::{Arc, Mutex};
use std::thread;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

pub fn run<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    err_fn: fn(cpal::StreamError),
    is_activated: Arc<Mutex<bool>>,
    ctx: Arc<Mutex<WhisperContext>>,
) where
    T: cpal::Sample + cpal::SizedSample + Into<f32>,
{
    // Testing code to check that the audio capture is working
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: config.sample_rate.0,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create("../../output.wav", spec).unwrap();
    let config_clone = config.clone();

    // Create a buffer to accumulate audio data
    let buffer = Arc::new(Mutex::new(Vec::new()));

    // Normal Code
    let stream = device
        .build_input_stream(
            &config_clone,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let is_activated = is_activated.lock().unwrap();
                if *is_activated {
                    // Convert the audio data to f32 and add it to the buffer
                    let mut buffer = buffer.lock().unwrap();
                    buffer.extend(data.iter().map(|sample| (*sample).into()));

                    // If the buffer has more than 2-3 seconds of audio data, process it
                    if buffer.len() >= (4 * config.sample_rate.0 as usize) {
                        let buffer_clone = buffer.clone();
                        for sample in buffer_clone {
                            let sample_f32: f32 = sample.into();
                            writer.write_sample(sample_f32).unwrap();
                        }
                        let ctx = ctx.lock().unwrap();
                        let mut state = ctx.create_state().expect("failed to create state");

                        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                        params.set_n_threads(2);
                        // params.set_translate(true);
                        params.set_print_special(false);
                        params.set_print_progress(false);
                        params.set_print_realtime(false);
                        params.set_print_timestamps(false);

                        let audio_data = whisper_rs::convert_stereo_to_mono_audio(&buffer).unwrap();

                        state.full(params, &audio_data).expect("failed to run model");

                        let num_segments = state.full_n_segments().expect("failed to get number of segments");
                        for i in 0..num_segments {
                            let segment = state.full_get_segment_text(i).expect("failed to get segment");
                            println!("Segment {}: {}", i, segment);

                            let start_timestamp = state
                                .full_get_segment_t0(i)
                                .expect("failed to get segment start timestamp");
                            let end_timestamp = state
                                .full_get_segment_t1(i)
                                .expect("failed to get segment end timestamp");
                            println!("[{} - {}]: {}", start_timestamp, end_timestamp, segment);
                        }

                        // Clear the buffer
                        buffer.clear();
                    }
                }
            },
            err_fn,
            None,
        )
        .unwrap();
    stream.play().unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
