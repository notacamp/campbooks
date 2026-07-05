# frozen_string_literal: true

module Digests
  # An item gathered from one source for inclusion in a digest issue.
  # `summary` is a short plain-text excerpt (~200 chars) used in the AI prompt;
  # the LLM never sees ids or URLs — items are numbered and mapped back by index.
  Item = Data.define(:source_type, :source_id, :title, :subtitle, :summary, :timestamp)
end
