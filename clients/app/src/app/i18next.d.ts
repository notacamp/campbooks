// Types `t()` keys against the English messages, one line per module.
import "i18next";

import type shell from "./i18n/en.json";
import type { ShortcutMessages } from "~/lib/shortcuts";
import type { UiMessages } from "~/lib/ui";
import type system from "~/modules/system/i18n/en.json";
import type conversations from "~/modules/conversations/i18n/en.json";
import type connectors from "~/modules/connectors/i18n/en.json";
import type workspaces from "~/modules/workspaces/i18n/en.json";
import type identity from "~/modules/identity/i18n/en.json";
import type attention from "~/modules/attention/i18n/en.json";
import type assistant from "~/modules/assistant/i18n/en.json";
import type search from "~/modules/search/i18n/en.json";
import type tasks from "~/modules/tasks/i18n/en.json";
import type schedule from "~/modules/schedule/i18n/en.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "shell";
    resources: {
      shell: typeof shell;
      ui: UiMessages;
      shortcuts: ShortcutMessages;
      system: typeof system;
      conversations: typeof conversations;
      connectors: typeof connectors;
      workspaces: typeof workspaces;
      identity: typeof identity;
      attention: typeof attention;
      assistant: typeof assistant;
      search: typeof search;
      tasks: typeof tasks;
      schedule: typeof schedule;
      // `cargo xtask new-module` inserts above this line.
    };
  }
}
