import { MemoryRouter as Router } from "react-router-dom";

import { QueryClientProvider } from "@tanstack/react-query";

import { queryClient } from "./api/constants";
import AppRoutes from "./routes";
import { AuthProvider } from "./store/auth-context";
// import { ReactQueryDevtools } from "@tanstack/react-query-devtools";

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Router>
        <AuthProvider>
          <AppRoutes />
        </AuthProvider>
      </Router>
      {/* <ReactQueryDevtools initialIsOpen /> */}
    </QueryClientProvider>
  );
}

export default App;
