// Dalaran houses: Lordaeron's composer and plan layer, with the platform_hall set and its hipped roofs. A layout is
// Lordaeron's layoutOf(p) with set "dalaran", roof "hip", and the eave margin; no chimney stack (the roof is unbroken).
import { compose, layoutOf, metaOf, registerSet } from "../lordaeron-ref/compose.mjs";
import { allModules, EAVE } from "./modules.mjs";

registerSet("dalaran", allModules);
export const dalaranLayout = (p) => ({ ...layoutOf({ ...p, roof: "gable_x" }), set: "dalaran", roof: "hip", margin: EAVE, chimney: null });
export { compose, metaOf };
