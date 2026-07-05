# frozen_string_literal: true

module Mcp
  # Serves narrative guides for the `guide` MCP tool. Guide content lives in
  # app/services/mcp/guides/*.md — Zeitwerk ignores non-.rb files, so they are
  # just data. load() memoizes reads so production pays one disk read per topic.
  module Guides
    GUIDES_DIR = File.expand_path("guides", __dir__)

    TOPICS = [
      { name: "getting_started",    summary: "What Campbooks is, the tool families, and recommended first calls." },
      { name: "triage_and_skim",    summary: "Skim deck, categories, awaiting-reply, and the learning loop." },
      { name: "organizing",         summary: "Folders, tags, document types, and cross-account provisioning." },
      { name: "documents",          summary: "Two-status axes, review flow, fields, upload, and filing." },
      { name: "tasks_and_calendar", summary: "Task statuses, create from email, reminders, events CRUD." },
      { name: "sending_email",      summary: "Send/reply/forward, account discovery, scheduled emails." },
      { name: "automation",         summary: "Workflows, scheduled emails, triggers, and templates." },
      { name: "setup_and_accounts", summary: "Connect email accounts (web vs token), AI providers, MCP keys." },
      { name: "context_tips",       summary: "Minimal-context patterns: counts, limits, batching, scope narrowing." }
    ].freeze

    @cache = {}

    module_function

    def load(topic)
      name = topic.to_s.strip
      return nil unless TOPICS.any? { |t| t[:name] == name }

      @cache[name] ||= begin
        path = File.join(GUIDES_DIR, "#{name}.md")
        File.exist?(path) ? File.read(path) : nil
      end
    end
  end
end
