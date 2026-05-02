import asyncio
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from kokoro import KPipeline
import soundfile as sf
import io
import torch

app = FastAPI()
pipeline = KPipeline(lang_code='a') 

# Helper to run the CPU-heavy generation in a separate thread
def generate_audio(text):
    generator = pipeline(text, voice="am_liam", speed=1.2, split_pattern=r'\n+')
    all_audio = [audio for _, _, audio in generator]
    if all_audio:
        combined_audio = torch.cat(all_audio)
        byte_io = io.BytesIO()
        sf.write(byte_io, combined_audio.numpy(), 24000, format='WAV')
        return byte_io.getvalue()
    return None

@app.websocket("/tts-stream")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    print("🟢 Rust backend connected")
    try:
        while True:
            text = await websocket.receive_text()
            # Offload heavy TTS to a thread so the WebSocket stays alive
            audio_bytes = await asyncio.to_thread(generate_audio, text)
            if audio_bytes:
                await websocket.send_bytes(audio_bytes)
    except WebSocketDisconnect:
        print("🔴 Rust backend disconnected")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8000)