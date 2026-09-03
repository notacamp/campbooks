# frozen_string_literal: true

require "pagy/extras/countless"

module People
  # Streams — the services' mail, grouped by kind (newsletters, receipts,
  # notifications, alerts). A stream is a workspace inbox group (Emails::TagGroups
  # over InboxGroupRule + grouped tags); the org page also opens a stream scoped to
  # one organization's service contacts (?organization_id=). The Streams tab shares
  # the People shell (PeopleLayout) — left pane is the stream list, right pane the
  # selected stream's threads (opening the classic thread in a top-level visit).
  class StreamsController < ApplicationController
    include PeopleLayout

    STREAM_THREADS_PER_PAGE = 25

    def people_active_tab = :streams

    def index
      build_streams_list
    end

    def show
      @stream_name = params[:name].to_s
      @organization = params[:organization_id].present? ? Current.workspace.organizations.find_by(id: params[:organization_id]) : nil

      base = stream_thread_scope
      @stream_total = base.count
      @stream_title = @organization ? "#{t("people.stream_kinds.#{@stream_name}")} · #{@organization.name}" : @stream_name
      @stream_icon = @organization ? org_kind_icon(@stream_name) : stream_icon(bucket_map[@stream_name])
      @stream_pagy, @stream_threads = pagy_countless(base, limit: STREAM_THREADS_PER_PAGE)

      respond_to do |format|
        format.turbo_stream # infinite-scroll append
        format.html do
          if turbo_frame_request_id == "people_detail"
            render :show_detail, layout: false
          else
            build_streams_list
            @selected_stream = @stream_name
            render :index
          end
        end
      end
    rescue Pagy::OverflowError
      respond_to do |format|
        format.turbo_stream { @stream_threads = []; @stream_pagy = nil; render :show }
        format.html { redirect_to people_stream_path(@stream_name, organization_id: @organization&.id) }
      end
    end

    private

    # ── Left pane: the workspace's streams (inbox groups) ─────────────────────
    def build_streams_list
      @streams = tag_groups_service.build_groups(inbox_folder_ids).map { |group| stream_summary(group) }
    end

    def stream_summary(group)
      name = group[:label]
      attention = group_attention_count(name)

      {
        name: name,
        icon: stream_icon(bucket_map[name]),
        count: group[:count],
        last_at: group_last_at(name),
        note: attention.positive? ? t("people.streams.may_need_you", count: attention) : t("people.streams.nothing_needs_you"),
        href: people_stream_path(name),
        selected: (@selected_stream == name)
      }
    end

    def bucket_map
      @bucket_map ||= Current.workspace.tags.where.not(default_bucket: nil).pluck(:group_name, :default_bucket).to_h
    end

    def stream_icon(bucket)
      case bucket
      when "notifications" then :bell
      when "promotions"    then :mail
      when "social"        then :users
      when "updates"       then :file
      else                      :mail
      end
    end

    def org_kind_icon(kind)
      case kind
      when "billing"       then :file
      when "notifications" then :bell
      when "newsletters"   then :mail
      when "social"        then :users
      else                      :file
      end
    end

    def group_attention_count(name)
      scope = tag_groups_service.group_scope(name)
      return 0 if scope.nil?

      scope.where(id: EmailMessage.where(category: "important").where.not(email_thread_id: nil).select(:email_thread_id)).distinct.count
    end

    def group_last_at(name)
      scope = tag_groups_service.group_scope(name)
      return nil if scope.nil?

      scope.joins(:email_messages).maximum("email_messages.received_at")
    end

    # ── Right pane: one stream's threads ──────────────────────────────────────
    def stream_thread_scope
      base = @organization ? org_stream_threads(@organization, @stream_name) : (tag_groups_service.group_scope(@stream_name) || EmailThread.none)

      base.where(email_account_id: readable_account_ids)
          .includes(:email_account, :email_messages)
          .order(Arel.sql("(SELECT MAX(m.received_at) FROM email_messages m WHERE m.email_thread_id = email_threads.id) DESC NULLS LAST"))
    end

    # The organization's service contacts of one stream kind, and their threads.
    def org_stream_threads(organization, kind)
      contact_ids = organization.contacts.kind_service.where(stream_kind: kind).ids
      return EmailThread.none if contact_ids.empty?

      thread_ids = EmailMessage.where(contact_id: contact_ids).where.not(email_thread_id: nil).distinct.pluck(:email_thread_id)
      EmailThread.where(id: thread_ids)
    end
  end
end
