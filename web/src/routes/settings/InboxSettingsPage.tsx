/**
 * InboxSettingsPage — Settings → Inbox settings at /settings/inbox.
 *
 * Organizes the six inbox-configuration surfaces as tabs:
 *   1. Tags       — visible / hidden tag management (create, rename, hide, delete)
 *   2. Rules      — inbox automation rules (create, toggle, run, delete)
 *   3. Signatures — email signatures (create, edit, set_default, delete)
 *   4. Doc types  — document-type classifiers (create, edit, delete)
 *   5. Tag groups — inbox stream groups (create, delete; edit via classic settings)
 *   6. Filtering  — inbox filter strategy + blocked/starred/allowed contacts
 *
 * Backend: GET/POST/PATCH/DELETE /api/app/inbox_settings/{tags,rules,signatures,
 *           document_types,tag_groups,filtering}
 * Hooks: ~/modules/settings/api/use-inbox-{tags,rules,signatures,document-types,
 *         tag-groups,filtering} (imported by direct path, NOT via the shared barrel)
 */
import { type FC, useState } from "react";
import { cn } from "~/lib/utils";
import { TagsSection } from "./inbox/TagsSection";
import { RulesSection } from "./inbox/RulesSection";
import { SignaturesSection } from "./inbox/SignaturesSection";
import { DocumentTypesSection } from "./inbox/DocumentTypesSection";
import { TagGroupsSection } from "./inbox/TagGroupsSection";
import { FilteringSection } from "./inbox/FilteringSection";

// ── Tab definitions ───────────────────────────────────────────────────────────

const TABS = [
  { id: "tags",           label: "Tags" },
  { id: "rules",          label: "Rules" },
  { id: "signatures",     label: "Signatures" },
  { id: "document-types", label: "Document types" },
  { id: "tag-groups",     label: "Tag groups" },
  { id: "filtering",      label: "Filtering" },
] as const;

type TabId = (typeof TABS)[number]["id"];

// ── TabBar ────────────────────────────────────────────────────────────────────

interface TabBarProps {
  active: TabId;
  onChange: (id: TabId) => void;
}

const TabBar: FC<TabBarProps> = ({ active, onChange }) => (
  <nav
    className="flex gap-1 overflow-x-auto pb-0.5 mb-6 border-b border-line scrollbar-none"
    aria-label="Inbox settings tabs"
  >
    {TABS.map((tab) => (
      <button
        key={tab.id}
        type="button"
        onClick={() => onChange(tab.id)}
        role="tab"
        aria-selected={active === tab.id}
        aria-controls={`inbox-tab-panel-${tab.id}`}
        id={`inbox-tab-${tab.id}`}
        className={cn(
          "shrink-0 px-3 py-2 rounded-cb-1 text-[13px] font-[500]",
          "transition-colors duration-[--cb-dur]",
          "focus:outline-none focus:ring-2 focus:ring-[--ring]",
          active === tab.id
            ? "bg-surface-2 text-t1"
            : "text-t3 hover:text-t2 hover:bg-surface-1",
        )}
      >
        {tab.label}
      </button>
    ))}
  </nav>
);

// ── InboxSettingsPage ─────────────────────────────────────────────────────────

export const InboxSettingsPage: FC = () => {
  const [activeTab, setActiveTab] = useState<TabId>("tags");

  return (
    <div
      data-testid="settings.page.root"
      className="max-w-2xl"
    >
      <h1 className="text-[17px] font-[650] text-t1 mb-5">Inbox settings</h1>

      <TabBar active={activeTab} onChange={setActiveTab} />

      <div
        role="tabpanel"
        id={`inbox-tab-panel-${activeTab}`}
        aria-labelledby={`inbox-tab-${activeTab}`}
      >
        {activeTab === "tags"           && <TagsSection />}
        {activeTab === "rules"          && <RulesSection />}
        {activeTab === "signatures"     && <SignaturesSection />}
        {activeTab === "document-types" && <DocumentTypesSection />}
        {activeTab === "tag-groups"     && <TagGroupsSection />}
        {activeTab === "filtering"      && <FilteringSection />}
      </div>
    </div>
  );
};
