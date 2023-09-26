import AppRoutes from "./routes";
import { QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter as Router } from "react-router-dom";
import { AuthProvider } from "./store/auth-context";
import { queryClient } from "./api/constants";

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Router>
        <AuthProvider>
          <AppRoutes />
        </AuthProvider>
      </Router>
    </QueryClientProvider>
  );
}

export default App;
