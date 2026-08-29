// The inventory table on the grammar reference index. Its own entry point,
// because that page has no parse table to draw and should not pay for the
// layout engine to say so.
import { statusOverview } from "./status.mjs";

for (const root of document.querySelectorAll(".status-overview")) {
  try {
    const response = await fetch("/status.json");
    if (!response.ok) throw new Error(`${response.status} fetching the inventory`);
    root.innerHTML = statusOverview(await response.json());
  } catch (error) {
    root.innerHTML = `<p class="broken">Could not load the inventory: ${
      String(error.message).replace(/[<&]/g, "")
    }</p>`;
  }
}
