import axios from "axios";
import { AppDispatch } from "../store/index";
import {
  getPublicKey,
  useRegistrationCode,
  pingAll,
  registrationError,
} from "../store/actions";
import { ThunkAction } from "redux-thunk";
import { Action } from "redux";
import { RootState } from "../store";
import { AppThunk } from "../types";
import { ShinkaiMessageBuilderWrapper } from "../lib/wasm/ShinkaiMessageBuilderWrapper";
import { MergedSetupType } from "../pages/Connect";

let API_ENDPOINT = "";

export const fetchPublicKey = () => async (dispatch: AppDispatch) => {
  try {
    const response = await axios.get(`${API_ENDPOINT}/get_public_key`);
    dispatch(getPublicKey(response.data));
  } catch (error) {
    console.error("Error fetching public key:", error);
  }
};

export const submitRegistrationCode =
  (setupData: MergedSetupType): AppThunk =>
  async (dispatch: AppDispatch) => {
    try {
      const messageStr = ShinkaiMessageBuilderWrapper.code_registration(
        setupData.myEncryptionSk,
        setupData.myIdentitySk,
        setupData.node_encryption_pk,
        setupData.registration_code,
        setupData.identity_type,
        setupData.permission_type,
        setupData.registration_name,
        "", // sender_profile_name: it doesn't exist yet in the Node
        setupData.shinkai_identity
      );

      const message = JSON.parse(messageStr);

      // Use node_address from setupData for API endpoint
      await axios.post(
        `${setupData.node_address}/v1/use_registration_code`,
        message
      );

      // Update the API_ENDPOINT after successful registration
      API_ENDPOINT = setupData.node_address;

      dispatch(useRegistrationCode(setupData));
    } catch (error) {
      let errorMessage = "Unexpected error occurred";

      if (error instanceof Error) {
        errorMessage = error.message;
      }

      dispatch(registrationError(errorMessage));
      console.error("Error using registration code:", error);
    }
  };

export const pingAllNodes = () => async (dispatch: AppDispatch) => {
  try {
    const response = await axios.post(`${API_ENDPOINT}/ping_all`);
    dispatch(pingAll(response.data.result));
  } catch (error) {
    console.error("Error pinging all nodes:", error);
  }
};
