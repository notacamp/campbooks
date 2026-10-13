// Types `t()` keys against the English messages, one line per module.
import "i18next";

import type shell from "./i18n/en.json";
import type system from "~/modules/system/i18n/en.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "shell";
    resources: {
      shell: typeof shell;
      system: typeof system;
      // `cargo xtask new-module` inserts above this line.
    };
  }
}
