// hooks/useSetup.ts
import { useEffect } from "react";
import { useSelector } from "react-redux";
import { RootState } from "../store";
import { ApiConfig } from "../api/api_config";

export const useSetup = () => {
  const { setupDetails } = useSelector((state: RootState) => state);

  useEffect(() => {
    console.log("Redux State:", setupDetails);
    ApiConfig.getInstance().setEndpoint(setupDetails.node_address);
  }, [setupDetails]);
};