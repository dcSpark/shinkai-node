// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{CustomMenuItem, SystemTray, SystemTrayEvent, SystemTrayMenu};
use tauri::{Manager, SystemTrayMenuItem};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

fn main() {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let hide_show = CustomMenuItem::new("hide_show".to_string(), "Hide");
    let activate_deactivate = CustomMenuItem::new("activate_deactivate".to_string(), "Activate");
    let create_task = CustomMenuItem::new("create_task".to_string(), "Create Task");
    let settings = CustomMenuItem::new("settings".to_string(), "Settings");

    let tray_menu = SystemTrayMenu::new()
        .add_item(hide_show)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(activate_deactivate.clone())
        .add_item(create_task)
        .add_item(settings)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);

    let system_tray = SystemTray::new().with_menu(tray_menu);

    let is_activated = Arc::new(Mutex::new(true));
    let is_activated_clone = Arc::clone(&is_activated);

    // Create a new WhisperContext
    let ctx = Arc::new(Mutex::new(
        WhisperContext::new("./models/ggml-tiny.bin").expect("failed to load model"),
    ));
    let ctx_clone = Arc::clone(&ctx);

    // Start a new thread for audio capture
    thread::spawn(move || {
        let host = cpal::default_host();
        let device = host
            .input_devices()
            .unwrap()
            .find(|d| d.name().unwrap() == "MacBook Pro Microphone")
            .expect("Failed to get MacBook Pro Microphone");

        println!("Selected input device: {}", device.name().unwrap());
        println!("Default input config: {:?}", device.default_input_config().unwrap());

        let config = device
            .default_input_config()
            .expect("Failed to get default input config");

        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        match config.sample_format() {
            cpal::SampleFormat::F32 => run::<f32>(&device, config.into(), err_fn, is_activated_clone, ctx_clone),
            cpal::SampleFormat::I16 => run::<i16>(&device, config.into(), err_fn, is_activated_clone, ctx_clone),
            cpal::SampleFormat::U16 => run::<u16>(&device, config.into(), err_fn, is_activated_clone, ctx_clone),
            _ => panic!("unsupported sample format"),
        }
    });

    tauri::Builder::default()
        .setup(|app| Ok(()))
        .system_tray(system_tray)
        .on_system_tray_event(move |app, event| match event {
            SystemTrayEvent::LeftClick {
                position: _, size: _, ..
            } => {
                println!("system tray received a left click");
            }
            SystemTrayEvent::RightClick {
                position: _, size: _, ..
            } => {
                println!("system tray received a right click");
            }
            SystemTrayEvent::DoubleClick {
                position: _, size: _, ..
            } => {
                println!("system tray received a double click");
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "hide_show" => {
                    let window = app.get_window("main").unwrap();
                    let menu_item = app.tray_handle().get_item("hide_show");
                    if window.is_visible().unwrap() {
                        window.hide().unwrap();
                        let _ = menu_item.set_title("Show");
                    } else {
                        window.show().unwrap();
                        window.center().unwrap();
                        let _ = menu_item.set_title("Hide");
                    }
                }
                "activate_deactivate" => {
                    let mut is_activated = is_activated.lock().unwrap();
                    let menu_item = app.tray_handle().get_item("activate_deactivate");
                    if *is_activated {
                        *is_activated = false;
                        let _ = menu_item.set_title("Activate");
                        println!("Feature is now deactivated");
                    } else {
                        *is_activated = true;
                        let _ = menu_item.set_title("Deactivate");
                        println!("Feature is now activated");
                    }
                }
                "create_task" => {
                    let window = app.get_window("main").unwrap();
                    window.emit("create_task", ()).unwrap();
                }
                "settings" => {
                    let window = app.get_window("main").unwrap();
                    window.emit("settings", ()).unwrap();
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn run<T>(
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
                    if buffer.len() >= (2 * config.sample_rate.0 as usize) {

                        let buffer_clone = buffer.clone();
                        for sample in buffer_clone {
                            let sample_f32: f32 = sample.into();
                            writer.write_sample(sample_f32).unwrap();
                        }
                        let ctx = ctx.lock().unwrap();
                        let mut state = ctx.create_state().expect("failed to create state");

                        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                        params.set_n_threads(1);
                        params.set_print_special(false);
                        params.set_print_progress(false);
                        params.set_print_realtime(false);
                        params.set_print_timestamps(false);

                        state.full(params, &buffer).expect("failed to run model");

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
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

// Previous

// // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
// #[tauri::command]
// fn greet(name: &str) -> String {
//     format!("Hello, {}! You've been greeted from Rust!", name)
// }

// fn main() {
//     tauri::Builder::default()
//         .invoke_handler(tauri::generate_handler![greet])
//         .run(tauri::generate_context!())
//         .expect("error while running tauri application");
// }
