import React from "react";
import useSnapToCorner from "./hooks/useSnapToCorner";
import myAvatar from "./assets/avatar.gif";
import RecordButton from "./components/RecordButton";
import useMacDictation from "./hooks/useMacDictation";

export default function App() {
  // snaps avatar to the bottom right corner of the screen
  useSnapToCorner();

  // toggles the button state 
  const { isRecording, toggleRecording } = useMacDictation();

  return (
    <div className="w-screen h-screen overflow-hidden flex flex-col items-center justify-center relative">
      <RecordButton 
        isRecording={isRecording} 
        onToggle={toggleRecording} 
      />
      <img 
        src={myAvatar} 
        alt="Desktop Companion" 
        className="drag-region w-full h-full object-contain cursor-grab active:cursor-grabbing p-4 pt-8"
      />
    </div>
  );
}