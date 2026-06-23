module Workflows
  # Trigger context for the generic `event` trigger. Wraps a domain Event and
  # exposes it to Liquid as `{{ event.* }}` (name, payload, subject, actor).
  #
  # It also bridges into the email/document machinery: when the event's subject
  # is an EmailMessage it surfaces as `email_message` (so the email_action step
  # works) and its documents flow into `documents` (so `document_type` conditions
  # work) — and likewise a Document subject becomes the single document. This is
  # what lets an event-triggered workflow reuse the existing email/document
  # actions and conditions unchanged.
  class EventContext < TriggerContext
    attr_reader :source_event

    def initialize(event)
      @event = event
      @source_event = event
    end

    def liquid_context
      {
        "event" => {
          "name" => @event.name,
          "occurred_at" => @event.occurred_at&.iso8601,
          "payload" => @event.payload || {},
          "subject" => subject_drop,
          "actor" => actor_drop
        }
      }
    end

    def trigger_data
      { "type" => "event", "event_id" => @event.id, "name" => @event.name }
    end

    def step_input
      { "event_id" => @event.id }
    end

    def subject
      @event.subject
    end

    # Surface an EmailMessage subject so email_action / send_email attachments work.
    def email_message
      subject if subject.is_a?(EmailMessage)
    end

    # Feed conditions: an email's documents, or the document itself.
    def documents
      case subject
      when EmailMessage then subject.documents
      when Document then [ subject ]
      else []
      end
    end

    private

    def subject_drop
      return nil unless @event.subject

      { "type" => @event.subject_type, "id" => @event.subject_id }
    end

    def actor_drop
      return nil unless @event.actor

      { "type" => @event.actor_type, "id" => @event.actor_id, "label" => actor_label }
    end

    def actor_label
      actor = @event.actor
      %i[name email_address email to_s].each do |attr|
        next unless actor.respond_to?(attr)

        value = actor.public_send(attr)
        return value.to_s if value.present?
      end
      nil
    end
  end
end
