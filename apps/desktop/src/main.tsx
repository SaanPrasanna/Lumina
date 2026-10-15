import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/app/App";
import { AppProviders } from "@/app/providers";
import "@/i18n";
// Sinhala fallback for systems without a Sinhala font (bundled, so it works
// offline); system fonts come first in --font-sans.
import "@fontsource/noto-sans-sinhala/sinhala-400.css";
import "@fontsource/noto-sans-sinhala/sinhala-500.css";
import "@fontsource/noto-sans-sinhala/sinhala-600.css";
import "@/styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppProviders>
      <App />
    </AppProviders>
  </React.StrictMode>,
);
