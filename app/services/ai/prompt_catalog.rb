module Ai
  # Single source of truth for the AI prompts a workspace can customize.
  #
  # Each entry maps a `purpose` — the exact key that
  # Ai::Configuration.user_prompt_suffix(purpose) reads — to the resource
  # "surface" it belongs to and an icon. The human-facing copy (label,
  # description, placeholder, example) lives in i18n under
  # `ai_prompts.catalog.<key>` so it stays translatable.
  #
  # This catalog is intentionally DECOUPLED from AiConfiguration::PURPOSES
  # (which governs model/adapter routing). A purpose can be customizable as a
  # prompt without having its own routing row — e.g. `task_extraction` routes
  # through the workspace's text model but still accepts custom guidance.
  #
  # Drives: the Settings → AI Prompts page, the per-resource "Customize AI"
  # modal, and AiPrompt's purpose validation.
  module PromptCatalog
    Entry = Data.define(:key, :surface, :icon) do
      def label       = I18n.t("ai_prompts.catalog.#{key}.label")
      def description = I18n.t("ai_prompts.catalog.#{key}.description")
      def placeholder = I18n.t("ai_prompts.catalog.#{key}.placeholder")
      def example     = I18n.t("ai_prompts.catalog.#{key}.example")
    end

    # Heroicon-style single-path `d` attributes (24x24, stroke).
    ICONS = {
      tasks:      "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01",
      documents:  "M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z",
      reminders:  "M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9",
      inbox:      "M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
    }.freeze

    ENTRIES = [
      Entry.new(key: "task_extraction",      surface: :tasks,     icon: ICONS[:tasks]),
      Entry.new(key: "document_analysis",    surface: :documents, icon: ICONS[:documents]),
      Entry.new(key: "reminder_extraction",  surface: :reminders, icon: ICONS[:reminders]),
      Entry.new(key: "email_analysis",       surface: :inbox,     icon: ICONS[:inbox]),
      Entry.new(key: "email_classification", surface: :inbox,     icon: ICONS[:inbox])
    ].freeze

    KEYS = ENTRIES.map(&:key).freeze

    def self.all = ENTRIES
    def self.find(key) = ENTRIES.find { |e| e.key == key.to_s }
    def self.key?(key) = KEYS.include?(key.to_s)
    def self.for_surface(surface) = ENTRIES.select { |e| e.surface == surface.to_sym }
  end
end
