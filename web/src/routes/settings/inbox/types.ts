/**
 * routes/settings/inbox/types — TypeScript shapes for the Inbox Settings API responses.
 *
 * Kept private to the inbox-settings subtree; NOT exported from modules/settings/types.ts.
 * Matches the backend serializers in app/serializers/api/app/settings/*.
 */

// ── Tags ──────────────────────────────────────────────────────────────────────

export interface Tag {
  id: number;
  name: string;
  color: string | null;
  hidden: boolean;
  kind: string;
  source: string;
  group_name: string | null;
  prompt: string | null;
  system_label: boolean;
  external: boolean;
  message_count: number | null;
  created_at: string;
}

/** Shape returned by GET /api/app/inbox_settings/tags */
export interface TagsData {
  visible: Tag[];
  hidden_system: Tag[];
  hidden_filtered: Tag[];
  pending_review_count: number;
}

export interface CreateTagParams {
  name: string;
  color?: string;
  prompt?: string;
  group_name?: string;
}

export interface UpdateTagParams {
  id: number;
  name?: string;
  color?: string;
  prompt?: string;
  group_name?: string;
}

// ── Rules ─────────────────────────────────────────────────────────────────────

export interface RuleCriteria {
  from?: string;
  to?: string;
  subject?: string;
  body?: string;
  category?: string[];
  email_account_id?: string;
  has_attachment?: boolean;
}

/** Shape returned by RuleSerializer */
export interface Rule {
  id: number;
  name: string;
  criteria: RuleCriteria;
  archive: boolean;
  mark_read: boolean;
  enabled: boolean;
  tag_ids: number[];
  mail_folder_id: number | null;
  last_run_at: string | null;
  created_at: string;
}

export interface CreateRuleParams {
  email_rule: {
    name: string;
    criteria: RuleCriteria;
    archive?: boolean;
    mark_read?: boolean;
    tag_ids?: number[];
    run_on_existing?: boolean;
  };
}

export interface UpdateRuleParams {
  id: number;
  email_rule: {
    name?: string;
    criteria?: RuleCriteria;
    archive?: boolean;
    mark_read?: boolean;
    tag_ids?: number[];
  };
}

// ── Signatures ────────────────────────────────────────────────────────────────

/** Shape returned by SignatureSerializer */
export interface Signature {
  id: number;
  name: string;
  content: string;
  is_default: boolean;
  email_account_ids: number[];
  created_at: string;
}

export interface CreateSignatureParams {
  signature: {
    name: string;
    content: string;
    is_default?: boolean;
  };
}

export interface UpdateSignatureParams {
  id: number;
  signature: {
    name?: string;
    content?: string;
    is_default?: boolean;
  };
}

// ── Document types ────────────────────────────────────────────────────────────

/** Shape returned by DocumentTypeSerializer */
export interface DocumentType {
  id: number;
  name: string;
  color: string | null;
  category: string | null;
  prompt: string | null;
  auto_star: boolean;
  created_at: string;
}

export interface CreateDocumentTypeParams {
  document_type: {
    name: string;
    color?: string;
    category?: string;
    prompt?: string;
    auto_star?: boolean;
  };
}

export interface UpdateDocumentTypeParams {
  id: number;
  document_type: {
    name?: string;
    color?: string;
    category?: string;
    prompt?: string;
    auto_star?: boolean;
  };
}

// ── Tag groups ────────────────────────────────────────────────────────────────

export interface TagGroupRule {
  rule_type: string;
  value: string;
}

/** Shape returned by GET /api/app/inbox_settings/tag_groups */
export interface TagGroup {
  name: string;
  tag_ids: number[];
  tag_names: string[];
  rules: TagGroupRule[];
}

export interface CreateTagGroupParams {
  name: string;
  tag_ids?: number[];
  rules?: TagGroupRule[];
}

export interface UpdateTagGroupParams {
  id: string; // URL-encoded group name
  name: string;
  original_name?: string;
  tag_ids?: number[];
  rules?: TagGroupRule[];
}

// ── Filtering ─────────────────────────────────────────────────────────────────

export interface FilterContact {
  id: number;
  name: string | null;
  email: string;
}

/** Shape returned by GET /api/app/inbox_settings/filtering */
export interface FilteringData {
  strategy: string;
  starred: FilterContact[];
  blocked: FilterContact[];
  allowed: FilterContact[];
}
