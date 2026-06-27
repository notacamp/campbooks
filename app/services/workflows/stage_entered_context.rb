# frozen_string_literal: true

module Workflows
  # Trigger context for when a pipeline item enters a stage. Exposes pipeline
  # and stage info plus item-specific fields so workflow steps can act on them.
  class StageEnteredContext < TriggerContext
    def initialize(pipeline_membership:, stage:)
      @membership = pipeline_membership
      @stage = stage
      @item = pipeline_membership.item
    end

    def liquid_context
      {
        "pipeline_name" => @membership.pipeline.name,
        "stage_name" => @stage.name,
        "stage_id" => @stage.id,
        "pipeline_id" => @membership.pipeline_id
      }.merge(item_context)
    end

    def trigger_data
      {
        "pipeline_name" => @membership.pipeline.name,
        "stage_name" => @stage.name,
        "pipeline_id" => @membership.pipeline_id,
        "stage_id" => @stage.id,
        "item_type" => @item.class.name,
        "item_id" => @item.id
      }
    end

    def step_input
      { "pipeline_membership_id" => @membership.id }
    end

    def subject
      @item
    end

    private

    def item_context
      if @item.is_a?(Document)
        {
          "document_title" => @item.display_title,
          "document_type" => @item.classification&.name
        }
      elsif @item.respond_to?(:subject)
        {
          "email_subject" => @item.subject,
          "email_from" => @item.from_address
        }
      else
        {}
      end
    end
  end
end
