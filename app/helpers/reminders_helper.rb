module RemindersHelper
  # Links to where a reminder was extracted from, so the user can jump back to the
  # evidence: an email-sourced reminder links to the email; a document-sourced one
  # links to the document AND each email it was attached to (if any).
  # → [{ kind: :email|:document, label:, path: }]
  def reminder_source_links(reminder)
    case reminder.source
    when EmailMessage
      [ reminder_source_link(:email, reminder.source) ]
    when Document
      doc = reminder.source
      [ reminder_source_link(:document, doc) ] +
        doc.email_messages.map { |email| reminder_source_link(:email, email) }
    else
      []
    end
  end

  def reminder_source_link(kind, record)
    path = kind == :email ? email_message_path(record) : document_path(record)
    { kind: kind, label: t("reminders.source.#{kind}"), path: path }
  end
end
