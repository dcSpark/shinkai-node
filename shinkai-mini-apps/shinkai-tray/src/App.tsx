import { listen } from "@tauri-apps/api/event";
import { register } from "@tauri-apps/api/globalShortcut";
import { useEffect, useState } from "react";
import Home from "./pages/Home";
import Settings from "./pages/Settings";
import CreateTaskView from "./pages/CreateTask";
import Onboarding from "./pages/Onboarding";
import { readTextFile, writeFile } from "@tauri-apps/api/fs";
import { invoke } from "@tauri-apps/api";

function App() {
  const [view, setView] = useState("home");
  const [taskInputVisible, setTaskInputVisible] = useState(false);
  const [isOnboardingCompleted, setIsOnboardingCompleted] = useState<boolean>(false);
  const [taskInput, setTaskInput] = useState("");

  useEffect(() => {
    listen("create_task", () => {
      setTaskInputVisible(true);
      setView("create_task");
      console.log("Create task event received");
    });
    listen("settings", () => setView("settings"));

    console.log("Registering hotkey");
    // Register the global shortcut
    register("Alt+Shift+Enter", async () => {
      console.log("Hotkey activated");
      // Get the window
      // const window = await Window.getCurrent();
      // Show the window
      // window.show();
    });

    // Check if setup data is valid
    (invoke("validate_setup_data") as Promise<boolean>)
      .then((isValid: boolean) => {
        setIsOnboardingCompleted(isValid);
        if (!isValid) {
          setView("onboarding");
        }
      })
      .catch((error: string) => {
        console.error("Failed to validate setup data:", error);
        setIsOnboardingCompleted(false);
        setView("onboarding");
      });
  }, []);

  const handleTaskInput = (event: React.ChangeEvent<HTMLInputElement>) => {
    setTaskInput(event.target.value);
  };

  const handleTaskSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    console.log("Task submitted: ", taskInput);
    setTaskInput("");
    setTaskInputVisible(false);
  };

  return (
    <div className="container">
      {!isOnboardingCompleted && <Onboarding setView={setView} setIsOnboardingCompleted={setIsOnboardingCompleted} />}
      {isOnboardingCompleted && view === "home" && <Home setView={setView} />}
      {isOnboardingCompleted && view === "settings" && <Settings setView={setView} />}
      {isOnboardingCompleted && view === "create_task" && <CreateTaskView setView={setView} />}
    </div>
  );
}

export default App;
