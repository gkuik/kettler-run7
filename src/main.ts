import { mount } from "svelte";
// Polices embarquées (l'appli marche hors ligne) : seules les graisses utilisées.
import "@fontsource/barlow/latin-400.css";
import "@fontsource/barlow/latin-600.css";
import "@fontsource/barlow/latin-700.css";
import "@fontsource/barlow-semi-condensed/latin-600.css";
import "./app.css";
import App from "./App.svelte";
import { browserMock, mock } from "./lib/api";
import { sim } from "./lib/sim.svelte";
import { treadmill } from "./lib/treadmill.svelte";

// Abonnement à l'état du tapis avant même le montage : aucun événement perdu.
void treadmill.init();
// Application lancée en mode simulateur ? (affiche alors le menu « Simulateur »)
void sim.init();

// Mode navigateur : le tapis simulé est accessible depuis la console
// (`kettlerMock.simulateLinkLoss()`, `.pullSafetyKey()`, `.failNextStopWrite()`,
// `.togglePulse()`).
if (browserMock) {
  (window as unknown as { kettlerMock: unknown }).kettlerMock = mock();
}

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
