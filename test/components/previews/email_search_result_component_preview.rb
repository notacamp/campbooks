# frozen_string_literal: true

class EmailSearchResultComponentPreview < ViewComponent::Preview
  Account = Struct.new(:color)
  Tag = Struct.new(:name, :color)

  # Lightweight stand-in for an EmailMessage so the preview needs no DB records.
  # Responds to everything Campbooks::EmailSearchResult reads, plus #to_param so
  # email_message_path resolves.
  class FakeMessage
    def initialize(**opts)
      @opts = opts
    end

    def to_param = @opts.fetch(:id, 1).to_s
    def read? = @opts.fetch(:read, true)
    def sent? = @opts.fetch(:sent, false)
    def has_attachment? = @opts.fetch(:has_attachment, false)
    def contact_id = @opts[:contact_id]
    def subject = @opts[:subject]
    def received_at = @opts[:received_at]
    def from_address = @opts[:from_address]
    def to_address = @opts[:to_address]
    def ai_summary = @opts[:ai_summary]
    def body = @opts[:body].to_s
    def category = @opts[:category]
    def tags = @opts.fetch(:tags, [])
    def email_account = @opts[:account]
  end

  def received
    render(Campbooks::EmailSearchResult.new(message: FakeMessage.new(
      subject: "Your March invoice is ready",
      from_address: "Stripe <billing@stripe.com>",
      ai_summary: "Invoice #1042 for $480.00 is attached and due in 14 days.",
      received_at: 3.hours.ago, read: true, has_attachment: true,
      category: "important", tags: [ Tag.new("Invoice", "#3b82f6") ], account: Account.new("#6366f1")
    )))
  end

  def unread
    render(Campbooks::EmailSearchResult.new(message: FakeMessage.new(
      subject: "Re: Project kickoff next week",
      from_address: "Jane Cooper <jane@acme.com>",
      ai_summary: "Jane confirms Tuesday 10am and asks for the agenda.",
      received_at: 20.minutes.ago, read: false,
      tags: [ Tag.new("Clients", "#10b981"), Tag.new("Urgent", "#ef4444") ], account: Account.new("#6366f1")
    )))
  end

  def sent
    render(Campbooks::EmailSearchResult.new(message: FakeMessage.new(
      subject: "Proposal v2",
      to_address: "client@bigco.com", sent: true, read: true,
      ai_summary: "Sent the revised proposal with updated pricing.",
      received_at: 1.day.ago, account: Account.new("#3b82f6")
    )))
  end

  def no_subject
    render(Campbooks::EmailSearchResult.new(message: FakeMessage.new(
      subject: nil, from_address: "noreply@news.example.com",
      body: "<p>Weekly digest of nothing in particular this week.</p>",
      received_at: 2.days.ago, read: true
    )))
  end
end
