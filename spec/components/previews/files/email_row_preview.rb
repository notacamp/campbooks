# frozen_string_literal: true

module Files
  # Preview for a filed-email list item (as shown inside a folder).
  class EmailRowPreview < Lookbook::Preview
    def default
      render(Campbooks::Files::EmailRow.new(email: sample, current_folder: nil))
    end

    private

    def sample
      EmailMessage.first ||
        EmailMessage.new(id: 1, subject: "Re: Contract draft", from_address: "alex@example.com", received_at: Time.current)
    end
  end
end
