import { listen } from "@tauri-apps/api/event";
import { register } from "@tauri-apps/api/globalShortcut";
import { useEffect, useState } from "react";
import Home from './pages/Home';
import Settings from './pages/Settings';
import CreateTaskView from "./pages/CreateTask";
import Onboarding from './pages/Onboarding';
import { readTextFile, writeFile } from '@tauri-apps/api/fs';

function App() {
  const [view, setView] = useState("home");
  const [taskInputVisible, setTaskInputVisible] = useState(false);
  const [isOnboardingCompleted, setIsOnboardingCompleted] = useState(false);
  const [taskInput, setTaskInput] = useState("");

  useEffect(() => {
    readTextFile('onboardingData.json')
    .then(data => {
      if (data) {
        setIsOnboardingCompleted(true);
      }
    })
    .catch(err => {
      console.error(err);
    });

    listen("create_task", () => {
      setTaskInputVisible(true);
      setView("create_task");
      console.log("Create task event received");
    });
    listen("settings", () => setView("settings"));

    console.log("Registering hotkey");
    // Register the global shortcut
    register("Alt+Enter", async () => {
      console.log("Hotkey activated");
      // Get the window
      // const window = await Window.getCurrent();
      // Show the window
      // window.show();
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

// import { useState } from "react";
// import reactLogo from "./assets/react.svg";
// import { invoke } from "@tauri-apps/api/tauri";
// import "./App.css";

// function App() {
//   const [greetMsg, setGreetMsg] = useState("");
//   const [name, setName] = useState("");

//   async function greet() {
//     // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
//     setGreetMsg(await invoke("greet", { name }));
//   }

//   return (
//     <div className="container">
//       <h1>Welcome to Tauri!</h1>

//       <div className="row">
//         <a href="https://vitejs.dev" target="_blank">
//           <img src="/vite.svg" className="logo vite" alt="Vite logo" />
//         </a>
//         <a href="https://tauri.app" target="_blank">
//           <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
//         </a>
//         <a href="https://reactjs.org" target="_blank">
//           <img src={reactLogo} className="logo react" alt="React logo" />
//         </a>
//       </div>

//       <p>Click on the Tauri, Vite, and React logos to learn more.</p>

//       <form
//         className="row"
//         onSubmit={(e) => {
//           e.preventDefault();
//           greet();
//         }}
//       >
//         <input
//           id="greet-input"
//           onChange={(e) => setName(e.currentTarget.value)}
//           placeholder="Enter a name..."
//         />
//         <button type="submit">Greet</button>
//       </form>

//       <p>{greetMsg}</p>
//     </div>
//   );
// }

// export default App;
