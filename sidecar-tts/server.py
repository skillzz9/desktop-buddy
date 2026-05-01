from fastapi import FastAPI, Response
from kokoro import KModel, KPipeline
import soundfile as sf
import io
import torch

app = FastAPI()

# 1. Initialize Pipeline (Downloads model from HuggingFace on first run)
# 'a' stands for American English
pipeline = KPipeline(lang_code='a') 

@app.get("/tts")
async def generate_tts(text: str, voice: str = "am_michael"):
    # 2. Generate audio tensors
    # Kokoro returns a generator of (graphemes, phonemes, audio_tensor)
    generator = pipeline(text, voice=voice, speed=1.4, split_pattern=r'\n+')
    
    # Collect all audio chunks
    all_audio = []
    for _, _, audio in generator:
        all_audio.append(audio)
    
    if not all_audio:
        return {"error": "No audio generated"}

    # 3. Concatenate and convert to WAV bytes
    combined_audio = torch.cat(all_audio)
    byte_io = io.BytesIO()
    sf.write(byte_io, combined_audio.numpy(), 24000, format='WAV')
    
    return Response(content=byte_io.getvalue(), media_type="audio/wav")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8000)