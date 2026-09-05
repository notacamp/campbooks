# frozen_string_literal: true

module People
  # What a message asks of the reader: Scout's read when the analyzer ran
  # (EmailMessage#ai_ask, a noun phrase), else the sender's own sentence
  # (Emails::AskExtractor, a quote). kind is :ai or :quote.
  Ask = Data.define(:text, :kind) do
    def self.for(message)
      return nil unless message

      ai = message.try(:ai_ask).to_s.strip
      return new(text: ai, kind: :ai) if ai.present?

      quote = Emails::AskExtractor.call(message)
      quote.present? ? new(text: quote, kind: :quote) : nil
    end
  end
end
