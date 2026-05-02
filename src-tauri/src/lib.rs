use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use reqwest::multipart;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use dotenvy::dotenv;
use std::env;
use std::io::Cursor; // Added for rodio playback
use futures_util::StreamExt; // Added for reading the Groq stream
use std::io::Write; // Added to flush print statements

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

        // === STEP 3: STREAMING LLM & TTS PIPELINE ===
        
        // 1. Channel from LLM to the async TTS Fetcher
        let (tx_text, mut rx_text) = tokio::sync::mpsc::channel::<String>(32);
        
        // 2. Channel from the async TTS Fetcher to the Sync Audio Player
        let (tx_audio, rx_audio) = std::sync::mpsc::channel::<Vec<u8>>();
        let tts_client = client.clone();

        // 3. Spawn the synchronous Audio Player on a dedicated thread
        // std::thread::spawn is safe for rodio because it doesn't move across threads.
        std::thread::spawn(move || {
            let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&stream_handle).unwrap();

            // Wait for downloaded audio bytes and queue them seamlessly
            while let Ok(audio_bytes) = rx_audio.recv() {
                if let Ok(source) = rodio::Decoder::new(Cursor::new(audio_bytes)) {
                    sink.append(source);
                }
            }
            // Once the channel closes, wait for the final queued audio to finish playing
            sink.sleep_until_end(); 
        });

        // 4. Spawn the asynchronous TTS Fetcher
tokio::spawn(async move {
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
    use futures_util::{StreamExt, SinkExt};

    let ws_url = "ws://127.0.0.1:8000/tts-stream";
    
    // Connect to the Python WebSocket
    if let Ok((ws_stream, _)) = connect_async(ws_url).await {
        let (mut ws_write, mut ws_read) = ws_stream.split();

        // Task to read binary audio coming BACK from Python
        let tx_audio_clone = tx_audio.clone();
        tokio::spawn(async move {
            while let Some(Ok(Message::Binary(bin))) = ws_read.next().await {
                let _ = tx_audio_clone.send(bin);
            }
        });

        // Loop to send text sentences TO Python
        while let Some(sentence) = rx_text.recv().await {
            if !sentence.trim().is_empty() {
                let _ = ws_write.send(Message::Text(sentence)).await;
            }
        }
    } else {
        eprintln!("❌ Could not connect to Python TTS WebSocket");
    }
});

        // Setup the payload to enable streaming
        let llm_payload = serde_json::json!({
            "model": "llama-3.1-8b-instant", 
            "messages": [
                {
                    "role": "system",
                    "content": "You are a gen z brainrot companion that talks only in current brainrot slang. You try and be gangster as well"
                },
                {
                    "role": "user",
                    "content": user_text
                }
            ],
            "stream": true // CRITICAL: Tell Groq to stream the response
        });

        let mut stream = client.post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(&api_key)
            .json(&llm_payload)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .bytes_stream();

        print!("🤖 AVATAR SAYS: ");
        let _ = std::io::stdout().flush();
        
        let mut sentence_buffer = String::new();

        // Iterate through the incoming text tokens
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            let data = String::from_utf8_lossy(&bytes);

            for line in data.lines() {
                if line.starts_with("data: [DONE]") { break; }
                if line.starts_with("data: ") {
                    let json_str = &line[6..];
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                            sentence_buffer.push_str(content);
                            
                            // Print to the terminal in real-time
                            print!("{}", content);
                            let _ = std::io::stdout().flush();

                            // Check for sentence delimiters to trigger the TTS
                            if content.contains('.') || content.contains('!') || content.contains('?') {
                                let to_speak = sentence_buffer.clone();
                                sentence_buffer.clear();
                                
                                // Send the finished sentence to the background TTS fetcher
                                let _ = tx_text.send(to_speak).await;
                            }
                        }
                    }
                }
            }
        }

        // Catch any remaining text that didn't end with a punctuation mark
        if !sentence_buffer.trim().is_empty() {
            let _ = tx_text.send(sentence_buffer).await;
        }

        println!("\n"); // Add a newline after the full stream finishes

    } else {
        println!("❌ API Error: {:?}", json);
    }

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