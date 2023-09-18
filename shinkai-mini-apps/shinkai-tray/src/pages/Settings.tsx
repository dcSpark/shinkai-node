import React from 'react';

interface SettingsProps {
  setView: (view: string) => void;
}

const Settings: React.FC<SettingsProps> = ({ setView }) => {
  return (
    <div>
      <button onClick={() => setView("home")}>Back</button>
      <h1>Settings</h1>
    </div>
  );
};

export default Settings;