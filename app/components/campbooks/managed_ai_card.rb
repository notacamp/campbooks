# frozen_string_literal: true

module Campbooks
  # The "Campbooks AI" status/confirmation card — shown when a cloud workspace runs
  # on platform-managed AI (no keys). Surfaced in onboarding, the setup modal, and
  # Settings → AI. Provider defaults come from Ai::Platform; they (and document
  # availability) can be passed in so the Lookbook preview can show every state
  # without depending on the environment's keys.
  class ManagedAiCard < Campbooks::Base
    # Brand names are proper nouns, not translatable copy.
    PROVIDER_LABELS = {
      "deepseek" => "DeepSeek",
      "openai" => "OpenAI",
      "anthropic" => "Anthropic",
      "gemini" => "Gemini"
    }.freeze

    # @param show_switch [Boolean] render the "use my own keys instead" link
    # @param switch_path [String, nil] href for that link
    # @param text_provider [String] provider key backing managed text
    # @param doc_provider [String] provider key backing managed documents
    # @param documents_available [Boolean, nil] override managed-doc availability (nil → ask Ai::Platform)
    def initialize(show_switch: false, switch_path: nil,
                   text_provider: Ai::Platform::MANAGED_TEXT_PROVIDER,
                   doc_provider: Ai::Platform::MANAGED_DOC_PROVIDER,
                   documents_available: nil, **attrs)
      @show_switch = show_switch
      @switch_path = switch_path
      @text_provider = text_provider
      @doc_provider = doc_provider
      @documents_available = documents_available.nil? ? Ai::Platform.documents_available? : documents_available
      @attrs = attrs
    end

    def view_template
      div(class: class_names("rounded-xl border border-accent-200 bg-accent-50/60 p-4 space-y-3", @attrs.delete(:class)), **@attrs) do
        div(class: "flex flex-wrap items-center gap-2") do
          h3(class: "text-sm font-semibold text-gray-900") { t(".title") }
          render(Campbooks::Badge.new(variant: :accent)) { t(".badge") }
        end

        p(class: "text-xs text-gray-600") { t(".body") }

        div(class: "space-y-1.5 rounded-lg bg-white/70 p-2.5 border border-accent-100") do
          provider_row(t(".text_role"), @text_provider)
          provider_row(t(".document_role"), @doc_provider) if @documents_available
        end

        unless @documents_available
          p(class: "text-xs text-amber-600") { t(".documents_unavailable") }
        end

        if @show_switch
          a(href: @switch_path, class: "inline-block text-xs font-medium text-accent-700 hover:text-accent-800 underline") { t(".switch_link") }
        end
      end
    end

    private

    def provider_row(role, provider)
      div(class: "flex items-center justify-between gap-2 text-xs") do
        span(class: "text-gray-500") { role }
        span(class: "font-medium text-gray-900") do
          plain PROVIDER_LABELS.fetch(provider, provider.titleize)
          span(class: "text-gray-400 ml-1") { AiConfiguration::DEFAULT_MODEL[provider] }
        end
      end
    end
  end
end
