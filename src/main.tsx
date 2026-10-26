import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AppProvider } from "./context/AppContext";
import "./locales";
import "./styles/globals.css";
import { installFrontendLog } from "./utils/frontendLog";
import { ErrorBoundary } from "./components/shared/ErrorBoundary";

installFrontendLog();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary scope="root">
      <AppProvider>
        <App />
      </AppProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
