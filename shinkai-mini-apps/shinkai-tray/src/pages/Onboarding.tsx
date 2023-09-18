import React, { Dispatch, SetStateAction, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";

interface OnboardingProps {
  setView: Dispatch<SetStateAction<string>>;
  setIsOnboardingCompleted: Dispatch<SetStateAction<boolean>>;
}

const Onboarding: React.FC<OnboardingProps> = ({
  setView,
  setIsOnboardingCompleted,
}) => {
  const [nodeAddress, setNodeAddress] = useState("http://localhost:13013");
  const [registrationCode, setRegistrationCode] = useState("");

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    console.log("Onboarding data submitted: ", nodeAddress, registrationCode);
    event.preventDefault();
    try {
      const response = await invoke("process_onboarding_data", {
        data: {
          node_address: nodeAddress,
          registration_code: registrationCode,
        },
      });
      console.log(response);

      // Update the state in the App component
    //   setIsOnboardingCompleted(true);
      setView("home");
    } catch (err) {
      console.error("Error invoking process_onboarding_data:", err);
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <label>
        Node Address:
        <input
          type="text"
          value={nodeAddress}
          onChange={(e) => setNodeAddress(e.target.value)}
        />
      </label>
      {/* <label>
        Registration Code:
        <input
          type="text"
          value={registrationCode}
          onChange={(e) => setRegistrationCode(e.target.value)}
        />
      </label> */}
      <input type="submit" value="Register" style={{ cursor: "pointer" }} />
    </form>
  );
};

export default Onboarding;
