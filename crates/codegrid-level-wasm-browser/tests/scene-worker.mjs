import init, {SceneLevelSession} from '/target/level-browser-bindings/codegrid_level_wasm_browser.js';
import {runSceneConformance} from './scene-conformance.mjs';
self.onmessage = async ({data}) => {
  try {
    await init();
    const session = new SceneLevelSession(data.profile);
    const results = runSceneConformance(text => session.request(text), data.cases,
      (condition, message) => { if (!condition) throw new Error(message); });
    session.shutdown();
    self.postMessage({passed: true, user_agent: self.navigator.userAgent, results});
  } catch (error) {
    self.postMessage({passed: false, error: String(error.stack ?? error)});
  }
};
