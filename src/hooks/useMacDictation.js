import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function useNativeDictation() {
  const [isRecording, setIsRecording] = useState(false);

  const toggleRecording = async () => {
    if (isRecording) {
      setIsRecording(false);
      // Tells Rust to stop the buffer and send to Groq
      await invoke("stop_recording_and_transcribe");
    } else {
      setIsRecording(true);
      // Tells Rust to start filling the buffer
      await invoke("start_recording");
    }
  };

  return { isRecording, toggleRecording };
}
