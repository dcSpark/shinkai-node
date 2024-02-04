import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import "./App.css";

type Settings = { [key: string]: string };
type ModelSelection = { [key: string]: boolean };

function App() {
  const [nodeStatus, setNodeStatus] = useState("");
  const [nodeRunning, setNodeRunning] = useState(false); // Added state to track if the node is running
  const [page, setPage] = useState("home");
  const [settings, setSettings] = useState<Settings>({});

  const [showPopup, setShowPopup] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [selectedModels, setSelectedModels] = useState<ModelSelection>({});

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

  async function toggleNode() {
    // Combined start and stop node functions
    if (nodeRunning) {
      await invoke("stop_shinkai_node");
      setNodeStatus("Server Stopped");
      setNodeRunning(false);
    } else {
      const result = await invoke("start_shinkai_node");
      setNodeStatus(result as string);
      setNodeRunning(true);
    }
  }

  async function scanLocalModels() {
    const models = await invoke<string[]>("scan_ollama_models");
    console.log(models); // Or handle the models list as needed
    const modelsStatus = models.reduce(
      (acc, model) => ({ ...acc, [model]: true }),
      {}
    );
    setModels(models);
    setSelectedModels(modelsStatus);
    setShowPopup(true);
  }

  async function checkNodeHealth() {
    const result = await invoke("check_node_health");
    console.log("nodes health: ", result); // Log the result for debugging (optional)
    setNodeStatus(result as string);
    // Update to check for both "Server Running" and pristine conditions
    setNodeRunning(
      result === "Server Running" ||
        result === "Node is pristine" ||
        result === "Node is not pristine"
    );
  }

  async function saveSettings() {
    for (let key in settings) {
      await invoke("set_env_var", { key, value: settings[key] });
    }
  }

  async function pruneServer() {
    await invoke("stop_node_and_delete_storage");
    setNodeStatus("Server Pruned and Stopped");
  }

  return (
    <div className="container">
      {showPopup && (
        <div className="popup-backdrop">
          <div className="popup">
            <span>Select Models</span>
            <span
              className="toggle-select"
              onClick={() => {
                const allSelected = Object.values(selectedModels).every(
                  (value) => value
                );
                const newSelections = Object.keys(selectedModels).reduce(
                  (acc, model) => ({ ...acc, [model]: !allSelected }),
                  {}
                );
                setSelectedModels(newSelections);
              }}
              style={{ cursor: "pointer" }}
            >
              {Object.values(selectedModels).some((value) => value)
                ? "Deselect All"
                : "Select All"}
            </span>
          </div>
          <div className="popup-content">
            {models.map((model) => (
              <div key={model}>
                <input
                  type="checkbox"
                  checked={selectedModels[model]}
                  onChange={() =>
                    setSelectedModels({
                      ...selectedModels,
                      [model]: !selectedModels[model],
                    })
                  }
                />
                {model}
              </div>
            ))}
          </div>
          <div className="popup-footer">
            <button onClick={() => setShowPopup(false)}>Back</button>
            <button
              onClick={() => {
                // Filter the selected models
                const selectedModelsList = Object.entries(selectedModels)
                  .filter(([_, isSelected]) => isSelected)
                  .map(([model, _]) => model);

                // Call the add_ollama_models function with the selected models
                invoke("add_ollama_models", { models: selectedModelsList })
                  .then((response) => {
                    console.log(response); // Handle the response as needed
                  })
                  .catch((error) => {
                    console.error("Failed to add models:", error); // Handle the error as needed
                  });

                setShowPopup(false); // Close the popup
              }}
            >
              Add
            </button>
          </div>
        </div>
      )}
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
          <p></p>
          <div className="start-button-container">
            <button
              className={nodeRunning ? "stop-button" : "start-button"}
              onClick={toggleNode}
            >
              {nodeRunning ? "Stop Node" : "Start Node"}
            </button>
            <button
              className="prune-button"
              onClick={pruneServer}
              style={{ backgroundColor: "red" }}
            >
              Prune Server
            </button>
            <button className="scan-models-button" onClick={scanLocalModels}>
              Scan for Local Models
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
