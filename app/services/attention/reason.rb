# frozen_string_literal: true

module Attention
  # One human-readable reason behind a weight: an i18n key under
  # attention.reasons plus its interpolation params. Serialized into
  # attention_weights.reasons as { "key" => ..., "params" => {...} }.
  Reason = Data.define(:key, :params) do
    # Reasons that raise a counterpart: the ones a Now card or a People lane row
    # may print. The rest (ignored, dismissed, service, taught_unimportant,
    # blocked, new, …) only ever surface in the People Details rail.
    POSITIVE_KEYS = %w[
      replies_fast replies two_way meetings invoices pays_promptly engaged
      starred allowed vip urgent_sender taught_important org_lead
    ].freeze

    def initialize(key:, params: {}) = super(key: key.to_s, params: params.to_h.transform_keys(&:to_s))
    def self.from_h(h) = new(key: h["key"] || h[:key], params: h["params"] || h[:params] || {})
    def to_h = { "key" => key, "params" => params }
    def sentence = I18n.t("attention.reasons.#{key}", **params.symbolize_keys)
    def positive? = POSITIVE_KEYS.include?(key)
  end
end
