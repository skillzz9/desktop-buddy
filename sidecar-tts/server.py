from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from kokoro import KPipeline
import soundfile as sf
import io
import torch

app = FastAPI()

# Initialize Pipeline (using 'am_liam' as an example for the brainrot vibe)
pipeline = KPipeline(lang_code='a') 

@app.websocket("/tts-stream")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    print("🟢 Rust backend connected to TTS WebSocket")
    
    try:
        while True:
            # 1. Wait for a sentence from Rust
            text = await websocket.receive_text()
            
            # 2. Generate audio
            generator = pipeline(text, voice="am_liam", speed=1.2, split_pattern=r'\n+')
            all_audio = []
            
            for _, _, audio in generator:
                all_audio.append(audio)
            
            if all_audio:
                # 3. Package into WAV bytes
                combined_audio = torch.cat(all_audio)
                byte_io = io.BytesIO()
                sf.write(byte_io, combined_audio.numpy(), 24000, format='WAV')
                
                # 4. Blast the binary audio straight back down the pipe
                await websocket.send_bytes(byte_io.getvalue())
                
    except WebSocketDisconnect:
        print("🔴 Rust backend disconnected")
        
if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8000)