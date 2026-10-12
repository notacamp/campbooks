import { createRoot } from "react-dom/client";

import "./styles.css";
import { App } from "./providers";

const container = document.querySelector("#root");
if (container === null) {
  throw new Error("Missing #root element in index.html");
}
createRoot(container).render(<App />);
