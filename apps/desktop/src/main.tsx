import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/app/App";
import { AppProviders } from "@/app/providers";
import "@/i18n";
// Manrope (text) and Roboto Mono (code, figures), bundled so they work offline.
import "@fontsource-variable/manrope";
import "@fontsource-variable/roboto-mono";
// Noto Sans Sinhala for all Sinhala text, every weight (variable, bundled).
import "@fontsource-variable/noto-sans-sinhala";
import "@/styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppProviders>
      <App />
    </AppProviders>
  </React.StrictMode>,
);
