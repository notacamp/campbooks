# frozen_string_literal: true

class Money
  # A single item in the "Needs you" section. Built by Money::Page.
  #
  # kind: :no_invoice | :review | :partial | :nif
  NeedsYouItem = Struct.new(
    :kind, :transaction, :match, :group, :title, :meta, :actions,
    keyword_init: true
  )
end
