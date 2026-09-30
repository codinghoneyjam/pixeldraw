import { Tool } from "./tool_base.js";

export class HandTool extends Tool {
  get id() {
    return "hand";
  }

  get cursor() {
    return "grab";
  }
}
