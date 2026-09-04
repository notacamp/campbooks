# frozen_string_literal: true

require "pagy/extras/countless"

module People
  # The organization page inside the People place: its people and its services
  # side by side. Shares the People shell (left pane = the counterpart list); the
  # right pane shows the org header, Scout's standing, "People at …" and "Streams
  # from …". Opened from a People-list org row into the "people_detail" frame.
  class OrganizationsController < ApplicationController
    include PeopleLayout

    def show
      @organization = Current.workspace.organizations.find(params[:id])
      build_org_detail

      respond_to do |format|
        format.html do
          if turbo_frame_request_id == "people_detail"
            render "people/show_detail", layout: false
          else
            build_people_list
            @selected_id = @organization.id
            render "people/index"
          end
        end
      end
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    private

    def build_org_detail
      people = org_detail_people
      ids    = people.map(&:id)

      # Read person rows from the standings table (fast, no email queries).
      rows_by_person_id = PeopleStanding.for_user(current_user)
                                        .where(counterpart_type: "Person", counterpart_id: ids)
                                        .index_by(&:counterpart_id)

      # Fall back to live computation for members not yet in the table.
      missing_people = people.reject { |p| rows_by_person_id.key?(p.id) }
      fallback_dir = missing_people.any? ? People::Directory.new(current_user, workspace: Current.workspace) : nil

      @org_people = people.map do |person|
        if (row = rows_by_person_id[person.id])
          row.to_counterpart
        else
          fallback_dir.counterpart_for(person)
        end
      end.sort_by { |c| -(c.last_activity&.to_i || 0) }

      # Org standing: from the table, else live.
      @standing = PeopleStanding.for_user(current_user).find_by(counterpart: @organization)&.standing ||
                  People::Standing.for_organization(@organization, user: current_user)

      @org_streams = org_stream_summaries
      @first_year  = @organization.email_messages.minimum(:received_at)&.year
      @doc_count   = @organization.documents.count
    end

    def org_detail_people
      @organization.active_people.includes(:contacts, :primary_organization)
                   .select { |person| person.contacts.any?(&:kind_person?) }
    end

    # One row per stream kind among the org's service contacts.
    def org_stream_summaries
      services = @organization.contacts.kind_service.to_a
      grouped = services.group_by { |contact| contact.stream_kind.presence || "notifications" }

      grouped.map do |kind, contacts|
        ids = contacts.map(&:id)
        latest = EmailMessage.where(contact_id: ids).accessible_to(current_user).order(received_at: :desc).first
        message_count = contacts.sum { |contact| contact.email_count.to_i }

        {
          kind: kind,
          icon: stream_kind_icon(kind),
          label: t("people.stream_kinds.#{kind}"),
          meta: t("people.index.service_meta", count: message_count),
          note: stream_note(latest),
          href: people_stream_path(kind, organization_id: @organization.id)
        }
      end.sort_by { |summary| summary[:label] }
    end

    def stream_kind_icon(kind)
      case kind
      when "billing"       then :file
      when "notifications" then :bell
      when "newsletters"   then :mail
      when "social"        then :users
      else                      :file
      end
    end

    def stream_note(message)
      return nil unless message

      summary = message.ai_summary.to_s.strip
      (summary.presence&.split(/(?<=[.!?])\s+/)&.first) || message.subject.presence
    end
  end
end
