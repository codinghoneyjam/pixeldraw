export class Tool {
  constructor(env = {}) {
    this.env = env;
    this.session = env.session ?? null;
    this.getView = env.getView ?? null;
    this.requestRender = env.requestRender ?? (() => {});
  }

  get id() {
    return "tool";
  }

  get cursor() {
    return "default";
  }

  activate() {}

  deactivate() {}

  pointerDown(_ev) {}

  pointerMove(_ev) {}

  pointerUp(_ev) {}

  cancel() {}

  hover(_ev) {}

  keyDown(_ev) {
    return false;
  }

  hasPending() {
    return false;
  }

  discardPending() {}

  overlay(_ctx, _info) {}
}
