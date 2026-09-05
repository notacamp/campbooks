# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::AskExtractor do
  def extractor(body: nil, summary: nil)
    message = instance_double(
      EmailMessage,
      body: body || "",
      summary: summary
    )
    described_class.call(message)
  end

  it "picks the last question after pleasantry" do
    body = "Hope you're well! Could you send the signed NDA by Friday?"
    expect(extractor(body: body)).to eq("Could you send the signed NDA by Friday?")
  end

  it "picks a request without a question mark" do
    body = "Thanks for your time. Please confirm the room booking."
    expect(extractor(body: body)).to eq("Please confirm the room booking.")
  end

  it "handles Portuguese question" do
    body = "Olá! Consegues enviar a fatura de junho?"
    expect(extractor(body: body)).to eq("Consegues enviar a fatura de junho?")
  end

  it "handles Spanish question with inverted mark" do
    body = "Hola. ¿Puedes confirmar la fecha?"
    expect(extractor(body: body)).to eq("¿Puedes confirmar la fecha?")
  end

  it "handles French request" do
    body = "Bonjour. Pourriez-vous confirmer le créneau ?"
    expect(extractor(body: body)).to include("Pourriez-vous confirmer")
  end

  it "never picks a question from the quoted history of an HTML reply" do
    body = '<p>Please send the invoice.</p><blockquote>Could you review the report?</blockquote>'
    expect(extractor(body: body)).to eq("Please send the invoice.")
  end

  it "keeps the sender's accents even when the ask is long enough to truncate" do
    body = "Olá! Poderias enviar-me a versão final da proposta com os anexos técnicos revistos, a certidão da empresa e a declaração de não dívida até sexta-feira?"
    result = extractor(body: body)
    expect(result).to start_with("Poderias enviar-me a versão final da proposta")
    expect(result).to end_with("…")
    expect(result).to include("técnicos").or include("certidão")
  end

  it "drops the lead-in before a dash or colon when the tail is the ask" do
    body = "Following up on your question about clause 7.2 — could you confirm the indemnity cap you'd accept?"
    expect(extractor(body: body)).to eq("Could you confirm the indemnity cap you'd accept?")
    expect(extractor(body: "Quick one: can you send the deck today?")).to eq("Can you send the deck today?")
  end

  it "keeps the whole sentence when the part after the dash is not an ask by itself" do
    body = "Which slot works for you — Tuesday or Wednesday?"
    expect(extractor(body: body)).to eq("Which slot works for you — Tuesday or Wednesday?")
  end

  it "treats a question closed by a typographic quote as a question, unwrapping the quote" do
    body = "Two things. Sofia wrote: “can you sign the NDA today?” Thanks!"
    expect(extractor(body: body)).to eq("Can you sign the NDA today?")
  end

  it "truncates long questions at word boundary" do
    long_ask = "Could you please review the very very very very very very very very very long document that we sent over last week by next Monday morning before the standup meeting starts?"
    result = extractor(body: long_ask)
    expect(result).not_to be_nil
    expect(result.length).to be <= 113  # MAX_LENGTH + ellipsis
    expect(result).to end_with("…")
  end

  it "returns nil for a pure FYI email" do
    body = "Just letting you know the report has been submitted. No action needed."
    expect(extractor(body: body)).to be_nil
  end

  it "picks from summary snippet when body is blank" do
    expect(extractor(body: "", summary: "Can you send the report?")).to eq("Can you send the report?")
  end

  it "returns nil for a pleasantry-only body like 'How are you?'" do
    expect(extractor(body: "How are you?")).to be_nil
  end
end
