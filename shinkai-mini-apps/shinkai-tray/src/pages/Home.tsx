import React from 'react';

interface HomeProps {
  setView: (view: string) => void;
}

const Home: React.FC<HomeProps> = ({ setView }) => {
  return (
    <div>
      <h1>Home</h1>
      <button onClick={() => setView("create_task")}>Create Task</button>
      <button onClick={() => setView("settings")}>Open Settings</button>
    </div>
  );
};

export default Home;