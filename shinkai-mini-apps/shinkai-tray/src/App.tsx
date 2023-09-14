import { listen } from "@tauri-apps/api/event";
import { register } from '@tauri-apps/api/globalShortcut'
import { useEffect, useState } from "react";

function App() {
  const [view, setView] = useState("home");

  useEffect(() => {
    listen("create_task", () => setView("create_task"));
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

  return (
    <div className="container">
      {view === "home" && <Home />}
      {view === "settings" && <Settings />}
      {view === "create_task" && <CreateTask />}
      {/* ... */}
      <button onClick={() => setView("settings")}>Open Settings</button>
      {/* ... */}
    </div>
  );
}

function Home() {
  return <h1>Home</h1>;
}

function Settings() {
  return <h1>Settings</h1>;
}

function CreateTask() {
  return <h1>Create Task</h1>;
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
