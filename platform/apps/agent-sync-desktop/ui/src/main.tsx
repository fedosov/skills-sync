import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { bootstrapPlatformAttributes } from "./lib/platform";
import "./index.css";

function disableWebviewContextMenu(target: Document = document): void {
  target.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });
}

async function bootstrap() {
  await bootstrapPlatformAttributes();
  disableWebviewContextMenu();
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap();
