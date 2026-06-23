class WorkflowStep < ApplicationRecord
  belongs_to :workflow

  has_many :execution_steps, class_name: "WorkflowExecutionStep", dependent: :destroy

  validates :position, presence: true, numericality: { only_integer: true, greater_than_or_equal_to: 0 }
  validates :step_type, presence: true, inclusion: { in: %w[condition action] }

  STEP_TYPES = %w[condition action].freeze

  # Action step types live in one place — Workflows::ActionRegistry — which the
  # executor, builder UI, and strong params all derive from. These constants are
  # projections of that registry, kept here for the model's validations and the
  # `heading` helper.
  ACTION_TYPES = Workflows::ActionRegistry.keys.freeze
  HTTP_ACTION_TYPES = Workflows::ActionRegistry.http_keys.freeze

  # Returns a hash of { action_type => localized label } for display purposes.
  # Reads from the activerecord.attributes.workflow_step.action_types locale
  # namespace, falling back to the registry's English label.
  def self.action_labels
    ACTION_TYPES.each_with_object({}) do |key, hash|
      hash[key] = I18n.t(
        "#{key}",
        scope: %i[activerecord attributes workflow_step action_types],
        default: Workflows::ActionRegistry.labels[key]
      )
    end
  end

  validates :action_type, inclusion: { in: ACTION_TYPES }, if: -> { step_type == "action" }
  validates :action_type, absence: true, if: -> { step_type == "condition" }

  scope :ordered, -> { order(:position) }

  def http_action?
    HTTP_ACTION_TYPES.include?(action_type)
  end

  # Human label for the step, e.g. "Send Email" or "Condition".
  def heading
    case step_type
    when "action"
      self.class.action_labels[action_type] || action_type.to_s.humanize
    else
      step_type.to_s.humanize
    end
  end
end
