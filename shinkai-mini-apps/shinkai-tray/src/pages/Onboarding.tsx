import React, { Dispatch, SetStateAction, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { APIUseRegistrationCodeSuccessResponse } from "../shinkai-message-ts/src/models/Payloads";
import { submitInitialRegistrationNoCode } from "../shinkai-message-ts/src/api";
import {
  generateEncryptionKeys,
  generateSignatureKeys,
} from "../shinkai-message-ts/src/utils/wasm_helpers";

interface OnboardingProps {
  setView: Dispatch<SetStateAction<string>>;
  setIsOnboardingCompleted: Dispatch<SetStateAction<boolean>>;
}

const Onboarding: React.FC<OnboardingProps> = ({ setView, setIsOnboardingCompleted }) => {
  const [nodeAddress, setNodeAddress] = useState("http://localhost:9550");
  // const [registrationCode, setRegistrationCode] = useState("");
  const [status, setStatus] = useState<"idle" | "loading" | "error" | "success">("idle");
  const [setupData, setSetupData] = useState({
    registration_code: "",
    profile: "main",
    registration_name: "main_device",
    identity_type: "device",
    permission_type: "admin",
    node_address: nodeAddress,
    shinkai_identity: "@@node1.shinkai", // this should actually be read from ENV
    node_encryption_pk: "",
    node_signature_pk: "",
    profile_encryption_sk: "",
    profile_encryption_pk: "",
    profile_identity_sk: "",
    profile_identity_pk: "",
    my_device_encryption_sk: "",
    my_device_encryption_pk: "",
    my_device_identity_sk: "",
    my_device_identity_pk: "",
  });

  useEffect(() => {
    fetch("http://127.0.0.1:9550/v1/shinkai_health")
      .then((response) => response.json())
      .then((data) => {
        if (data.status === "ok") {
          setSetupData((prevState) => ({
            ...prevState,
            node_address: "http://127.0.0.1:9550",
          }));
        }
      })
      .catch((error) => console.error("Error:", error));
  }, []);

  // Generate keys when the component mounts
  useEffect(() => {
    // Assuming the seed is a random 32 bytes array.
    // Device Keys
    let seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(
      ({ my_encryption_sk_string, my_encryption_pk_string }) =>
        setSetupData((prevState) => ({
          ...prevState,
          my_device_encryption_pk: my_encryption_pk_string,
          my_device_encryption_sk: my_encryption_sk_string,
        }))
    );
    generateSignatureKeys().then(({ my_identity_pk_string, my_identity_sk_string }) =>
      setSetupData((prevState) => ({
        ...prevState,
        my_device_identity_pk: my_identity_pk_string,
        my_device_identity_sk: my_identity_sk_string,
      }))
    );

    // Profile Keys
    seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(
      ({ my_encryption_sk_string, my_encryption_pk_string }) =>
        setSetupData((prevState) => ({
          ...prevState,
          profile_encryption_pk: my_encryption_pk_string,
          profile_encryption_sk: my_encryption_sk_string,
        }))
    );
    generateSignatureKeys().then(({ my_identity_pk_string, my_identity_sk_string }) =>
      setSetupData((prevState) => ({
        ...prevState,
        profile_identity_pk: my_identity_pk_string,
        profile_identity_sk: my_identity_sk_string,
      }))
    );
  }, []);

  const finishSetup = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setStatus("loading");
    let success = false;
    const response = await submitInitialRegistrationNoCode(setupData);
    success = response.success;
    const responseData: APIUseRegistrationCodeSuccessResponse | undefined = response.data;

    if (success) {
      let updatedSetupData = { ...setupData };
      if (responseData) {
        updatedSetupData = {
          ...updatedSetupData,
          node_encryption_pk: responseData.encryption_public_key,
          node_signature_pk: responseData.identity_public_key,
        };
      }

      // Pass updatedSetupData to the Rust backend
      try {
        const response = await invoke("process_onboarding_data", {
          data: updatedSetupData,
        });
        console.log(response);
        setIsOnboardingCompleted(true);
      } catch (err) {
        console.error("Error invoking process_onboarding_data:", err);
      }

      setStatus("success");
      setView("home");
    } else {
      setStatus("error");
    }
  };

  return (
    <form onSubmit={finishSetup}>
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
