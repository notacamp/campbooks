# frozen_string_literal: true

class EmailListItemComponentPreview < ViewComponent::Preview
  def received
    message = preview_message(
      from_address: "maria.silva@example.com",
      to_address: "user@not-a-camp.com",
      subject: "Invoice #42 for Q1 consulting",
      ai_summary: "Maria sent the quarterly invoice for consulting services with updated rates.",
      sent: false
    )
    render(Campbooks::EmailListItem.new(message: message))
  end

  def sent
    message = preview_message(
      from_address: "user@not-a-camp.com",
      to_address: "maria.silva@example.com",
      subject: "Re: Invoice #42 - Payment confirmed",
      ai_summary: nil,
      sent: true
    )
    render(Campbooks::EmailListItem.new(message: message))
  end

  def with_tags
    message = preview_message(
      from_address: "john.doe@partner.org",
      to_address: "user@not-a-camp.com",
      subject: "Contract renewal for 2026",
      ai_summary: "John sent the updated contract with revised terms for the upcoming year.",
      sent: false
    ).tap { |m|
      m.define_singleton_method(:tags) do
        [
          OpenStruct.new(name: "Important", color: "#EF4444"),
          OpenStruct.new(name: "Contracts", color: "#8B5CF6")
        ]
      end
    }
    render(Campbooks::EmailListItem.new(message: message))
  end

  def no_summary
    message = preview_message(
      from_address: "newsletter@somecompany.com",
      to_address: "user@not-a-camp.com",
      subject: "Weekly digest - May 2026",
      ai_summary: nil,
      sent: false
    )
    render(Campbooks::EmailListItem.new(message: message))
  end

  private

  def preview_message(from_address:, to_address:, subject:, ai_summary:, sent:)
    tags = []
    OpenStruct.new(
      from_address: from_address,
      to_address: to_address,
      subject: subject,
      ai_summary: ai_summary,
      received_at: Time.current - rand(1..14).days,
      sent?: sent,
      tags: tags
    )
  end
end
