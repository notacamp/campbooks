# frozen_string_literal: true

module Onboarding
  # Applies one or more setup templates to a workspace. All operations are
  # idempotent and additive: existing tags and document types are never deleted
  # or overwritten. When multiple templates are combined, the union of their
  # tags and document types is created, and a module is visible if ANY selected
  # template marks it visible. User-configured module visibility always wins.
  #
  # Usage (single):
  #   result = Onboarding::TemplateApplier.new(workspace, "freelancer").apply!
  #
  # Usage (multiple):
  #   result = Onboarding::TemplateApplier.new(workspace, %w[freelancer personal_admin]).apply!
  #   result[:tags]           # => union of Tag records created or found
  #   result[:document_types] # => union of DocumentType records created or found
  class TemplateApplier
    class UnknownTemplate < ArgumentError; end

    def initialize(workspace, template_keys)
      @workspace     = workspace
      @template_keys = Array(template_keys).map(&:to_s).reject(&:empty?)
      @templates     = @template_keys.filter_map { |k| Templates.find(k) }

      invalid = @template_keys - Templates.keys
      raise UnknownTemplate, "Unknown template keys: #{invalid.inspect}" if invalid.any?
    end

    # Applies all templates and returns a result hash with :tags and :document_types.
    def apply!
      tags           = provision_tags
      document_types = provision_document_types
      apply_module_visibility
      record_choice

      { tags: tags, document_types: document_types }
    end

    private

    # Union of all tag definitions across the selected templates.
    def provision_tags
      seen_names = Set.new
      @templates.flat_map { |tpl| tpl[:tags] }.filter_map do |attrs|
        name = attrs[:name].strip.downcase
        next if seen_names.include?(name)

        seen_names << name
        @workspace.tags.find_by(name: name) ||
          @workspace.tags.create!(
            name:   name,
            color:  attrs[:color],
            prompt: attrs[:prompt],
            source: :local,
            kind:   :user,
            hidden: false
          )
      rescue ActiveRecord::RecordInvalid, ActiveRecord::RecordNotUnique
        @workspace.tags.find_by(name: name)
      end
    end

    # Union of all document type definitions across the selected templates.
    def provision_document_types
      seen_names = Set.new
      @templates.flat_map { |tpl| tpl[:document_types] }.filter_map do |attrs|
        name = attrs[:name].strip.downcase
        next if seen_names.include?(name)

        seen_names << name
        @workspace.document_types.find_by(name: name) ||
          @workspace.document_types.create!(
            name:     name,
            color:    attrs[:color],
            category: attrs[:category],
            prompt:   attrs[:prompt]
          )
      rescue ActiveRecord::RecordInvalid, ActiveRecord::RecordNotUnique
        @workspace.document_types.find_by(name: name)
      end
    end

    # A module is visible if ANY selected template marks it visible.
    # User-configured keys always win over the template-derived value.
    def apply_module_visibility
      known_keys = Templates::CATALOG.flat_map { |t| t[:module_visibility].keys }.uniq

      # Compute union visibility: true if any template enables the module.
      union = known_keys.index_with do |k|
        @templates.any? { |tpl| tpl[:module_visibility].fetch(k, true) != false }
      end

      # Merge so user overrides win (user value replaces the union value for that key).
      current = @workspace.settings["module_visibility"] || {}
      @workspace.settings["module_visibility"] = union.merge(current)
    end

    def record_choice
      @workspace.settings["setup_templates"] = @template_keys
      @workspace.save!
    end
  end
end
