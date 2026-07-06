# frozen_string_literal: true

module Onboarding
  # Applies a setup template to a workspace. All operations are idempotent and
  # additive: existing tags and document types are never deleted or overwritten.
  # Module visibility settings are set but can be toggled by the user afterwards.
  #
  # Usage:
  #   result = Onboarding::TemplateApplier.new(workspace, "freelancer").apply!
  #   result[:tags]           # => array of Tag records created or found
  #   result[:document_types] # => array of DocumentType records created or found
  class TemplateApplier
    class UnknownTemplate < ArgumentError; end

    def initialize(workspace, template_key)
      @workspace    = workspace
      @template_key = template_key.to_s
      @template     = Templates.find(@template_key)
      raise UnknownTemplate, "No template with key #{@template_key.inspect}" unless @template
    end

    # Applies the template and returns a result hash with :tags and :document_types.
    def apply!
      tags           = provision_tags
      document_types = provision_document_types
      apply_module_visibility
      record_choice

      { tags: tags, document_types: document_types }
    end

    private

    def provision_tags
      @template[:tags].filter_map do |attrs|
        name = attrs[:name].strip.downcase
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

    def provision_document_types
      @template[:document_types].filter_map do |attrs|
        name = attrs[:name].strip.downcase
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

    def apply_module_visibility
      visibility = @template[:module_visibility] || {}
      current    = @workspace.settings["module_visibility"] || {}
      # Only set keys not already explicitly configured by the user.
      merged = visibility.merge(current)
      @workspace.settings["module_visibility"] = merged
    end

    def record_choice
      @workspace.settings["setup_template"] = @template_key
      @workspace.save!
    end
  end
end
