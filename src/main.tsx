import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

window.addEventListener(
  "contextmenu",
  (e) => {
    const target = e.target as HTMLElement | null;
    if (
      target &&
      target.closest(
        'input:not([readonly]):not([type="checkbox"]):not([type="radio"]):not([type="range"]), textarea:not([readonly]), [contenteditable="true"]'
      )
    ) {
      return;
    }
    e.preventDefault();
  },
  true
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
