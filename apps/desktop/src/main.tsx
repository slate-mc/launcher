import * as Sentry from "@sentry/react";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { FatalError } from "./components/FatalError";
import "./styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("slate could not find its application root.");
}

createRoot(root).render(
  <StrictMode>
    <Sentry.ErrorBoundary fallback={<FatalError />}>
      <App />
    </Sentry.ErrorBoundary>
  </StrictMode>,
);
