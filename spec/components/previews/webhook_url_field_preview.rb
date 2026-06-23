# frozen_string_literal: true

class WebhookUrlFieldPreview < Lookbook::Preview
  # The URL is set — shows the read-only field with a copy button.
  def with_url
    render(Campbooks::WebhookUrlField.new(
      url: "https://campbooks.not-a-camp.com/webhooks/Xq7s2_kP9aB3cD4eF5gH6iJ7",
      hint: "Send a POST request with a JSON body to this URL to run the workflow."
    ))
  end

  # No URL yet — shows the pending hint instead.
  def pending
    render(Campbooks::WebhookUrlField.new(url: nil))
  end

  # Custom label.
  def custom_label
    render(Campbooks::WebhookUrlField.new(
      url: "https://campbooks.not-a-camp.com/webhooks/abc123",
      label: "Your inbound endpoint"
    ))
  end
end
