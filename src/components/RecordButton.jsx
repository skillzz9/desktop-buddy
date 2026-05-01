import React from "react";

export default function RecordButton({ isRecording, onToggle }) {
  return (
    <button 
      onClick={onToggle}
      className={`absolute top-2 z-10 px-4 py-1 rounded-full text-xs font-bold no-drag cursor-pointer transition ${
        isRecording 
          ? "bg-red-500 text-white animate-pulse shadow-[0_0_15px_rgba(239,68,68,0.7)]" 
          : "bg-slate-800/80 text-white hover:bg-slate-700"
      }`}
    >
      {isRecording ? "Stop Recording" : "Record"}
    </button>
  );
}