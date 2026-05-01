import { useEffect } from "react";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { invoke } from "@tauri-apps/api/core";

export default function useSnapToCorner() {
  useEffect(() => {
    async function snap() {
      try {
        const appWindow = getCurrentWindow();
        const monitor = await currentMonitor();

        if (monitor) {
          const scaleFactor = monitor.scaleFactor;
          const physicalScreenWidth = monitor.size.width;
          const physicalScreenHeight = monitor.size.height;

          // prints monitor dimensions to terminal
          await invoke("log_to_terminal", {
            msg: `Monitor found: ${physicalScreenWidth}x${physicalScreenHeight} (Scale: ${scaleFactor})`,
          });

          // This code currently does not work properly but thats okay cause its to do with the GIF dimensions for the avatar
          // which we can change ------------------------------------------------------------------------------------------- //
          const physicalWindowWidth = 300 * scaleFactor;
          const physicalWindowHeight = 240 * scaleFactor;
          const padding = 20 * scaleFactor;
          const x = physicalScreenWidth - physicalWindowWidth - padding;
          const y = physicalScreenHeight - physicalWindowHeight - padding;
          await invoke("log_to_terminal", {
            msg: `Moving window to X: ${x}, Y: ${y}`,
          });
          // --------------------------------------------------------------------------------------------------------------- //

          await appWindow.setPosition(new PhysicalPosition(x, y));
          await appWindow.show();

          // error
          await invoke("log_to_terminal", {
            msg: "Window successfully moved and shown!",
          });
        } else {
          await invoke("log_to_terminal", {
            msg: "ERROR: Could not find the monitor!",
          });
        }
      } catch (error) {
        await invoke("log_to_terminal", {
          msg: `FATAL ERROR: ${error.message || error}`,
        });
      }
    }
    // run the actual function
    snap();
  }, []);
}
