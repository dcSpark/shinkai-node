import { useEffect, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/tauri";
import "./App.css";

type Settings = { [key: string]: string };

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [nodeStatus, setNodeStatus] = useState("");
  const [page, setPage] = useState("home");
  const [settings, setSettings] = useState<Settings>({});

  useEffect(() => {
    // Get the current settings when the component mounts
    invoke<Settings>("get_settings").then((fetchedSettings) => {
      console.log(fetchedSettings);
      setSettings(fetchedSettings);
    });

    // Start the interval to check node health
    const intervalId = setInterval(checkNodeHealth, 2000);

    // Clear the interval when the component unmounts
    return () => clearInterval(intervalId);
  }, []);

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    setGreetMsg(await invoke("greet", { name }));
  }

  async function startNode() {
    const result = await invoke("start_shinkai_node");
    setNodeStatus(result as string);
  }

  async function checkNodeHealth() {
    const result = await invoke("check_node_health");
    setNodeStatus(result as string);
  }

  async function saveSettings() {
    for (let key in settings) {
      await invoke("set_env_var", { key, value: settings[key] });
    }
  }

  async function stopNode() {
    const result = await invoke("stop_shinkai_node");
    setNodeStatus(result as string);
  }

  return (
    <div className="container">
      <div className="tab-bar">
        <button onClick={() => setPage("home")}>Home</button>
        <button onClick={() => setPage("settings")}>Settings</button>
      </div>

      {page === "home" && (
        <>
          <h1>Welcome to Shinkai Node!</h1>
          <div className="row">
            <a href="https://vitejs.dev" target="_blank">
              <img src="/vite.svg" className="logo vite" alt="Shinkai Logo" />
            </a>
          </div>
          // TODO: Check if the node has ever been started before
          <p></p>
          <div className="start-button-container">
            <button className="start-button" onClick={startNode}>
              Start Node
            </button>
            <button className="stop-button" onClick={stopNode}>
              Stop Node
            </button>
          </div>
          <p>{nodeStatus}</p>
        </>
      )}
      {page === "settings" && (
        <>
          <h1>Settings</h1>
          <>
            <form
              className="settings-grid"
              onSubmit={(e) => {
                e.preventDefault();
                saveSettings();
              }}
            >
              {Object.keys(settings).map((key) => (
                <label key={key}>
                  <span className="key-name">{key}:</span>
                  <input
                    value={settings[key] || ""}
                    onChange={(e) =>
                      setSettings({ ...settings, [key]: e.target.value })
                    }
                  />
                </label>
              ))}
            </form>
            {/* Separate container for the save button */}
            <div className="save-button-container">
              <button type="submit" onClick={saveSettings}>
                Save
              </button>
            </div>
          </>
        </>
      )}
    </div>
  );
}

export default App;
