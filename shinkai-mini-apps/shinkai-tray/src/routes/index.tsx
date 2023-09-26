import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import {
  ADD_AGENT_PATH,
  CREATE_CHAT_PATH,
  CREATE_JOB_PATH,
  ONBOARDING_PATH,
  SETTINGS_PATH,
} from "./name";
import { useAuth } from "../store/auth-context";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";

import MainLayout from "../pages/layout/main-layout";
import CreateAgentPage from "../pages/create-agent";
import CreateChatPage from "../pages/create-chat";
import CreateJobPage from "../pages/create-job";
import OnboardingPage from "../pages/onboarding";
import SettingsPage from "../pages/settings";
import { ApiConfig } from "@shinkai_network/shinkai-message-ts/api";
import ChatLayout from "../pages/chat/layout";
import EmptyMessage from "../pages/chat/empty-message";
import ChatConversation from "../pages/chat/chat-conversation";

const ProtectedRoute = ({ children }: { children: React.ReactNode }) => {
  const { setupData } = useAuth();
  const { pathname } = useLocation();
  console.log("pathname ", pathname);

  useEffect(() => {
    ApiConfig.getInstance().setEndpoint(setupData?.node_address ?? "");
  }, [setupData?.node_address]);

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
          path="inboxes/*"
          element={
            <ProtectedRoute>
              <ChatLayout />
            </ProtectedRoute>
          }
        >
          <Route index element={<EmptyMessage />} />
          <Route path=":inboxId" element={<ChatConversation />} />
        </Route>
        <Route
          path={ADD_AGENT_PATH}
          element={
            <ProtectedRoute>
              <CreateAgentPage />
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
      <Route path="/" element={<Navigate to={"inboxes/"} replace />} />
    </Routes>
  );
};
export default AppRoutes;
