import asyncio
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from kokoro import KPipeline
import soundfile as sf
import io
import base64
import pyautogui
import torch

app = FastAPI()
pipeline = KPipeline(lang_code='a') 

def sync_screenshot():
    # This part runs in a separate thread to avoid blocking FastAPI
    return pyautogui.screenshot()


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


@app.get("/capture")
async def capture_screen():
    print("📸 Incoming capture request...")
    try:
        screenshot = await asyncio.to_thread(sync_screenshot)
        
        buffered = io.BytesIO()
        screenshot.save(buffered, format="PNG")
        img_str = base64.b64encode(buffered.getvalue()).decode("utf-8")
        
        print("✅ Screenshot captured successfully.")
        return {"b64_image": img_str}
    except Exception as e:
        print(f"❌ Screenshot failed: {e}")
        return {"error": str(e)}, 500

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8000)