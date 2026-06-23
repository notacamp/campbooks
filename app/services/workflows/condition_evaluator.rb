module Workflows
  # Evaluates a condition step against a TriggerContext.
  #
  # Two flavours of condition share one config shape (field / operator / value):
  #   - "document_type" — a structured check over the email's documents
  #   - any other field — treated as a Liquid expression (e.g. "payload.status"
  #     or "{{ email.subject }}") rendered against the context, then compared.
  class ConditionEvaluator
    OPERATORS = %w[equals not_equals contains not_contains exists not_exists].freeze

    def self.evaluate(step, context)
      new(step, context).evaluate
    end

    def initialize(step, context)
      @config = step.config.with_indifferent_access
      @context = context
    end

    def evaluate
      case @config[:field].to_s
      when "document_type"
        evaluate_document_type
      when ""
        true
      else
        evaluate_expression
      end
    end

    private

    def evaluate_document_type
      target = @config[:value].to_s.downcase.strip
      return true if target.blank?

      doc_types = @context.documents.map { |d|
        (d.classification&.name || d.document_type)&.downcase
      }.compact

      case @config[:operator]
      when "not_equals" then doc_types.none? { |t| t == target }
      when "contains" then doc_types.any? { |t| t.include?(target) }
      else doc_types.any? { |t| t == target } # equals / default
      end
    end

    def evaluate_expression
      actual = render_field(@config[:field].to_s)
      target = @config[:value].to_s

      case @config[:operator]
      when "not_equals"  then actual.strip != target.strip
      when "contains"    then actual.downcase.include?(target.downcase)
      when "not_contains" then !actual.downcase.include?(target.downcase)
      when "exists"      then actual.present?
      when "not_exists"  then actual.blank?
      else actual.strip == target.strip # equals / default
      end
    end

    # Accepts either a bare path ("payload.event") or a full Liquid expression
    # ("{{ payload.event }}"). A render error resolves to an empty string so a
    # malformed expression simply fails its comparison rather than the workflow.
    def render_field(field)
      template = field.include?("{{") ? field : "{{ #{field} }}"
      LiquidRenderer.new(@context.liquid_context).render(template).to_s
    rescue LiquidRenderer::Error
      ""
    end
  end
end
