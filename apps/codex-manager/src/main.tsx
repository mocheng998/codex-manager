import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");

if (root instanceof HTMLElement) {
  createRoot(root).render(<App />);
}

