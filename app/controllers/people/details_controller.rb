# frozen_string_literal: true

module People
  # The Details rail / sheet for a single person inside the People place.
  # Opened lazily into turbo-frame "people_details" from the conversation pane.
  #
  # Write actions (rename / relationship / kind / state / analyze / merge) re-render
  # the whole frame so the UI is always consistent with the record. Each action
  # delegates to the same mechanisms the classic ContactsController uses.
  class DetailsController < ApplicationController
    include PeopleLayout

    before_action :set_person

    # GET /people/:id/details
    def show
      @profile = People::Profile.for(@person, user: current_user)
      render_details
    end

    # PATCH /people/:id/details/rename
    def rename
      @person.update!(name: params.require(:name).to_s.strip.presence || @person.name)
      after_write
    end

    # PATCH /people/:id/details/relationship
    def relationship
      @person.update!(relationship_type: params[:relationship_type].presence)
      after_write
    end

    # PATCH /people/:id/details/kind
    def kind
      kind_val = params[:kind].to_s
      return head(:unprocessable_entity) unless Contact.sender_kinds.key?(kind_val)

      contact = primary_contact
      return head(:not_found) unless contact

      stream = kind_val == "service" ? Contacts::StreamKind.classify(contact) : nil
      contact.update!(sender_kind: kind_val, sender_kind_source: "taught", stream_kind: stream)
      Organizations::FromDomain.link(contact) if contact.kind_service?
      Events.publish("contact.sender_kind_taught", subject: contact,
        payload: { "name" => contact.name, "email" => contact.email, "sender_kind" => kind_val })

      after_write
    end

    # PATCH /people/:id/details/state
    def state
      contact = primary_contact
      return head(:not_found) unless contact

      toast_key = nil
      undo_endpoint = nil

      case params[:state]
      when "star"
        contact.star!
        toast_key = "people.details.toasts.starred"
        undo_endpoint = undo_people_details_path(@person)
      when "unstar"
        contact.unstar!
        toast_key = "people.details.toasts.unstarred"
        undo_endpoint = undo_people_details_path(@person)
      when "allow"
        contact.allow!
        toast_key = "people.details.toasts.allowed"
      when "block"
        Contacts::Block.call(contact, user: current_user)
        toast_key = "people.details.toasts.blocked"
        undo_endpoint = unblock_people_details_path(@person)
      when "unblock"
        Contacts::Unblock.call(contact, user: current_user)
        toast_key = "people.details.toasts.unblocked"
      else
        return head(:unprocessable_entity)
      end

      after_write(toast_key: toast_key, undo_endpoint: undo_endpoint)
    end

    # PATCH /people/:id/details/undo
    def undo
      contact = primary_contact
      return head(:not_found) unless contact

      contact.unstar! if contact.starred?
      after_write(toast_key: "people.details.toasts.unstarred")
    end

    # PATCH /people/:id/details/unblock
    def unblock
      contact = primary_contact
      return head(:not_found) unless contact

      Contacts::Unblock.call(contact, user: current_user)
      after_write(toast_key: "people.details.toasts.unblocked")
    end

    # POST /people/:id/details/analyze
    def analyze
      return if require_ai_provider!(:text)

      contact = primary_contact
      ContactAnalysisJob.perform_later(contact.id, force: true) if contact
      after_write(toast_key: "people.details.toasts.analysis_queued")
    end

    # POST /people/:id/details/merge
    def merge
      contact = primary_contact
      return head(:not_found) unless contact

      if params[:approve] == "true"
        target = contact.suggested_person
        contact.update!(person: target, suggested_person_id: nil,
                        suggested_reason: nil, suggested_confidence: nil)
        after_write(toast_key: "people.details.toasts.merged", redirect_to: target)
      else
        contact.update!(suggested_person_id: nil, suggested_reason: nil, suggested_confidence: nil)
        after_write(toast_key: "people.details.toasts.merge_dismissed")
      end
    end

    private

    def set_person
      @person = Current.workspace.people.find(params[:id])
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    def primary_contact
      @person.contacts.order(email_count: :desc).first
    end

    def after_write(toast_key: nil, undo_endpoint: nil, redirect_to: nil)
      People::Standings.refresh_counterpart!(current_user, @person)

      target_person = redirect_to || @person
      @profile = People::Profile.for(target_person, user: current_user)

      respond_to do |format|
        format.turbo_stream do
          streams = [ turbo_stream.replace("people_details", render_to_string(
            "people/details/show", locals: { profile: @profile }, layout: false
          )) ]
          if toast_key
            message = t(toast_key)
            toast = if undo_endpoint
              render_to_string(
                Campbooks::ActionToast.new(
                  message: message, variant: :success,
                  undo: { endpoint: undo_endpoint, label: t("people.actions.undo") }
                ),
                layout: false
              )
            else
              render_to_string(
                Campbooks::ActionToast.new(message: message, variant: :success),
                layout: false
              )
            end
            streams << turbo_stream.append(Campbooks::ActionToast::REGION_ID, toast)
          end
          render turbo_stream: streams
        end
        format.html { redirect_to person_page_path(@person) }
      end
    end

    def render_details
      respond_to do |format|
        format.html do
          render "people/details/show", locals: { profile: @profile }, layout: false
        end
      end
    end
  end
end
