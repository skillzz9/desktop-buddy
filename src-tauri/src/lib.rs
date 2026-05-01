use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use reqwest::multipart;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use dotenvy::dotenv;
use std::env;

// state to share data between background audio thread and React
struct AppState {
    is_recording: Arc<Mutex<bool>>,
    audio_samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

// simple command for running debug logs from react into terminal 
#[tauri::command]
fn log_to_terminal(msg: String) {
    println!("React says: {}", msg);
}

// React calls this to start capturing
//----------------------------------------------------------------//
#[tauri::command]
fn start_recording(state: State<AppState>) {
    state.audio_samples.lock().unwrap().clear();
    *state.is_recording.lock().unwrap() = true;
    // debugging line
    println!("🎤 Rust: Microphone active, recording PCM data...");
}
//----------------------------------------------------------------//


// FUNCTION THAT STOPS THE RECORDING AND SENDS TO GROQ API
//-------------------------------------------------------------------------------------------//
#[tauri::command]
async fn stop_recording_and_transcribe(state: State<'_, AppState>) -> Result<(), String> {
    *state.is_recording.lock().unwrap() = false;
    println!("⏹️ Rust: Recording stopped. Packaging audio...");

    // getting the audio and saving it to local variable
    let samples = state.audio_samples.lock().unwrap().clone();

    // error handling for no audio 
    if samples.is_empty() {
        println!("⚠️ No audio captured.");
        return Ok(());
    }

    // Change audio into .wav 
    let spec = WavSpec {
        channels: state.channels,
        sample_rate: state.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    // Save to local hard disk
    let filepath = "/tmp/virtualfriend_audio.wav";
    let mut writer = WavWriter::create(filepath, spec).map_err(|e| e.to_string())?;
    for sample in samples {
        writer.write_sample(sample).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    println!("☁️ Sending PCM payload to Groq API...");

    // Load the .env file and safely get the secure API key
    dotenv().ok(); 
    let api_key = env::var("GROQ_API_KEY")
        .map_err(|_| "CRITICAL ERROR: GROQ_API_KEY not found in .env file".to_string())?;

    let file_bytes = std::fs::read(filepath).map_err(|e| e.to_string())?;
    let part = multipart::Part::bytes(file_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav").unwrap();

    let form = multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-large-v3-turbo"); // choosing whisper to be fastest model

    // CALL API
    let client = reqwest::Client::new();
    let res = client.post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(&api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

    // Print text to terminal and trigger the LLM
    if let Some(text_value) = json.get("text") {
        let user_text = text_value.as_str().unwrap_or("");
        println!("\n========================================");
        println!("🗣️ YOU SAID: {}", user_text);
        println!("========================================\n");

        println!("🧠 Avatar is thinking...");

        // === LLM STEP ===
        let llm_payload = serde_json::json!({
            "model": "llama-3.1-8b-instant", 
            "messages": [
                {
                    "role": "system",
                    "content": "You are a posh, fancy, traditional CEO from EY who thinks they are the top level. He never admits hes wrong always thinks hes right."
                },
                {
                    "role": "user",
                    "content": user_text
                }
            ]
        });

        let llm_res = client.post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(&api_key)
            .json(&llm_payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let llm_json: serde_json::Value = llm_res.json().await.map_err(|e| e.to_string())?;

        // Extract the LLM's response and print it
        if let Some(choices) = llm_json.get("choices") {
            if let Some(first_choice) = choices.get(0) {
                if let Some(message) = first_choice.get("message") {
                    if let Some(content) = message.get("content") {
                        let avatar_reply = content.as_str().unwrap_or("");
                        println!("🤖 AVATAR SAYS: {}\n", avatar_reply);
                        return Ok(());
                    }
                }
            }
        }
        println!("❌ LLM Error: {:?}", llm_json);

    } else {
        println!("❌ API Error: {:?}", json);
    }

    // run the function
    Ok(())
}
//-------------------------------------------------------------------------------------------//


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Boot up the Mac audio drivers
            let host = cpal::default_host();
            let device = host.default_input_device().expect("No audio input device found");
            let config = device.default_input_config().unwrap();

            let sample_rate = config.sample_rate().0;
            let channels = config.channels();

            let is_recording = Arc::new(Mutex::new(false));
            let audio_samples = Arc::new(Mutex::new(Vec::new()));

            let is_rec_clone = is_recording.clone();
            let samples_clone = audio_samples.clone();

            // Start the infinite background thread
            std::thread::spawn(move || {
                let stream = match config.sample_format() {
                    cpal::SampleFormat::F32 => device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &_| {
                            // Only save data if the React button is pressed
                            if *is_rec_clone.lock().unwrap() {
                                samples_clone.lock().unwrap().extend_from_slice(data);
                            }
                        },
                        |err| eprintln!("Stream error: {}", err),
                        None
                    ),
                    _ => panic!("Only f32 format is supported on this Mac"),
                }.unwrap();

                stream.play().unwrap();

                // Keep the thread alive forever
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            });

            // Make the state available to our React commands
            app.manage(AppState {
                is_recording,
                audio_samples,
                sample_rate,
                channels,
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        // Register the new commands
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording_and_transcribe,
            log_to_terminal
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}