# frozen_string_literal: true

# View helpers for the People place. Small formatting bits shared by the
# conversation and organization views.
module PeopleHelper
  # The author label on a conversation bubble: "You" for your own messages, else
  # the sender's display name (the contact's, falling back to the address).
  def people_bubble_name(message)
    return t("people.conversation.you") if message.sent?

    message.contact&.display_name.presence || message.from_address.to_s.split("@").first.to_s.tr(".", " ").titleize
  end

  # A person's first name, for "Reply to Sofia".
  def people_first_name(person)
    person.display_name.to_s.strip.split(/\s+/).first.presence || person.display_name
  end

  # The conversation header meta line: "Brightloop · sofia@… · 14 emails since March".
  def people_conversation_meta(person, primary_contact, email_total, first_seen)
    parts = [ person.organization_name.presence, primary_contact&.email ]
    if email_total.to_i.positive? && first_seen
      parts << t("people.conversation.emails_since", count: email_total, since: l(first_seen.to_date, format: :section))
    end
    parts.compact.join(" · ")
  end
end
