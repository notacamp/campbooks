import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: "category",
      label: "Getting Started",
      items: [
        "getting-started/overview",
        "getting-started/installation",
      ],
    },
    {
      type: "category",
      label: "Deployment",
      items: [
        "deployment/overview",
        "deployment/self-hosting",
      ],
    },
    {
      type: "category",
      label: "Email",
      items: [
        "email/connecting-accounts",
        "email/scanning",
      ],
    },
    {
      type: "category",
      label: "Documents",
      items: [
        "documents/overview",
        "documents/classification",
      ],
    },
    {
      type: "category",
      label: "AI",
      items: [
        "ai/configuration",
      ],
    },
  ],
};

export default sidebars;
