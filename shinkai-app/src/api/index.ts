import axios from "axios";
import { AppDispatch } from "../store/index";
import {
  getPublicKey,
  useRegistrationCode,
  pingAll,
  createRegistrationCode,
  registrationError,
} from "../store/actions";
import { ThunkAction } from "redux-thunk";
import { Action } from "redux";
import { RootState } from "../store";
import { AppThunk } from "../types";
import { ShinkaiMessageBuilderWrapper } from "../lib/wasm/ShinkaiMessageBuilderWrapper";
import { MergedSetupType } from "../pages/Connect";
import { useSelector } from "react-redux";
import { ApiConfig } from "./api_config";

export const fetchPublicKey = () => async (dispatch: AppDispatch) => {
  const apiEndpoint = ApiConfig.getInstance().getEndpoint();
  try {
    const response = await axios.get(`${apiEndpoint}/get_public_key`);
    dispatch(getPublicKey(response.data));
  } catch (error) {
    console.error("Error fetching public key:", error);
  }
};

export const submitCreateRegistrationCode =
  (identity_permissions: string, code_type = "profile") =>
  async (dispatch: AppDispatch) => {
    const apiEndpoint = ApiConfig.getInstance().getEndpoint();
    try {
      const response = await axios.post(
        `${apiEndpoint}/v1/create_registration_code`,
        {
          // Identity permissions are: "admin", "standard" and "none"
          permissions: identity_permissions,
          // "device" or "profile"
          code_type,
        }
      );
      dispatch(createRegistrationCode(response.data.code));
    } catch (error) {
      console.error("Error creating registration code:", error);
    }
  };

export const submitRegistrationCode =
  (setupData: MergedSetupType): AppThunk =>
  async (dispatch: AppDispatch) => {
    try {
      const messageStr = ShinkaiMessageBuilderWrapper.use_code_registration(
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
      console.log("Message:", message);

      // Use node_address from setupData for API endpoint
      let _ = await axios.post(
        `${setupData.node_address}/v1/use_registration_code`,
        message
      );

      // Update the API_ENDPOINT after successful registration
      ApiConfig.getInstance().setEndpoint(setupData.node_address);

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
  const apiEndpoint = ApiConfig.getInstance().getEndpoint();
  try {
    const response = await axios.post(`${apiEndpoint}/ping_all`);
    dispatch(pingAll(response.data.result));
  } catch (error) {
    console.error("Error pinging all nodes:", error);
  }
};
