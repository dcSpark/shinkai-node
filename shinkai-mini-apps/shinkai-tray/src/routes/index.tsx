import { Navigate, Route, Routes } from "react-router-dom";
import {
  ADD_AGENT_PATH,
  CREATE_CHAT_PATH,
  CREATE_JOB_PATH,
  HOME_PATH,
  ONBOARDING_PATH,
  SETTINGS_PATH,
} from "./name";
import { useAuth } from "../store/auth-context";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";

import MainLayout from "../pages/layout/main-layout";
import HomePage from "../pages/home";
import AddAgentPage from "../pages/add-agent";
import CreateChatPage from "../pages/create-chat";
import CreateJobPage from "../pages/create-job";
import OnboardingPage from "../pages/onboarding";
import SettingsPage from "../pages/settings";

const ProtectedRoute = ({ children }: { children: React.ReactNode }) => {
  const { setupData } = useAuth();
  if (!setupData) {
    return <Navigate to={ONBOARDING_PATH} replace />;
  }
  return children;
};

const AppRoutes = () => {
  useEffect(() => {
    console.log("Registering hotkey");
    // Register the global shortcut
    // register("Alt+Shift+Enter", async () => {
    //   console.log("Hotkey activated");
    // });

    // Check if setup data is valid
    (invoke("validate_setup_data") as Promise<boolean>)
      .then((isValid: boolean) => {
        console.log("is already", isValid);
      })
      .catch((error: string) => {
        console.error("Failed to validate setup data:", error);
      });
  }, []);

  return (
    <Routes>
      <Route element={<MainLayout />}>
        <Route path={ONBOARDING_PATH} element={<OnboardingPage />} />
        <Route
          path={HOME_PATH}
          element={
            <ProtectedRoute>
              <HomePage />
            </ProtectedRoute>
          }
        />
        <Route
          path={ADD_AGENT_PATH}
          element={
            <ProtectedRoute>
              <AddAgentPage />
            </ProtectedRoute>
          }
        />
        <Route
          path={CREATE_CHAT_PATH}
          element={
            <ProtectedRoute>
              <CreateChatPage />
            </ProtectedRoute>
          }
        />
        <Route
          path={CREATE_JOB_PATH}
          element={
            <ProtectedRoute>
              <CreateJobPage />
            </ProtectedRoute>
          }
        />
        <Route
          path={SETTINGS_PATH}
          element={
            <ProtectedRoute>
              <SettingsPage />
            </ProtectedRoute>
          }
        />
      </Route>
    </Routes>
  );
};
export default AppRoutes;
