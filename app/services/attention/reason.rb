# frozen_string_literal: true

module Attention
  # One human-readable reason behind a weight: an i18n key under
  # attention.reasons plus its interpolation params. Serialized into
  # attention_weights.reasons as { "key" => ..., "params" => {...} }.
  Reason = Data.define(:key, :params) do
    def initialize(key:, params: {}) = super(key: key.to_s, params: params.to_h.transform_keys(&:to_s))
    def self.from_h(h) = new(key: h["key"] || h[:key], params: h["params"] || h[:params] || {})
    def to_h = { "key" => key, "params" => params }
    def sentence = I18n.t("attention.reasons.#{key}", **params.symbolize_keys)
  end
end
