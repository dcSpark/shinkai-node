import React, { useState } from "react";

interface CreateTaskViewProps {
  setView: (view: string) => void;
}

const CreateTaskView: React.FC<CreateTaskViewProps> = ({ setView }) => {
  const [taskInput, setTaskInput] = useState("");

  const handleTaskInput = (event: React.ChangeEvent<HTMLInputElement>) => {
    setTaskInput(event.target.value);
  };

  const handleTaskSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    console.log("Task submitted: ", taskInput);
    setTaskInput("");
    setView("home");
  };

  return (
    <div>
      <button onClick={() => setView("home")}>Back</button>
      <form onSubmit={handleTaskSubmit}>
        <input type="text" value={taskInput} onChange={handleTaskInput} />
      </form>
    </div>
  );
};

export default CreateTaskView;