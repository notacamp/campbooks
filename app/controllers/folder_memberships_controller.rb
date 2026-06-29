class FolderMembershipsController < ApplicationController
  before_action :require_authentication

  # File content into a folder (the Files "filesystem" layer). The folderable is
  # one of Document / AuthoredDocument / EmailMessage, resolved workspace-scoped
  # (emails through the readable-accounts gate) — never trust a client-supplied
  # type/id to reach another model or workspace. folderable_type defaults to
  # Document so the existing documents page (which posts only folderable_id) is
  # unchanged.
  def create
    folder = Current.workspace.mail_folders.find(params[:mail_folder_id])
    folderable = resolve_folderable!
    membership = folder.folder_memberships.find_or_create_by!(folderable: folderable)
    publish_membership_event(folderable, folder, "filed") if membership.previously_new_record?

    respond_after(folderable)
  end

  def destroy
    membership = FolderMembership.joins(:mail_folder)
                                 .where(mail_folders: { workspace_id: Current.workspace.id })
                                 .find(params[:id])
    folderable = membership.folderable
    folder = membership.mail_folder
    membership.destroy
    publish_membership_event(folderable, folder, "unfiled") if folderable

    respond_after(folderable)
  end

  private

  ALLOWED_FOLDERABLE_TYPES = %w[Document AuthoredDocument EmailMessage].freeze

  def resolve_folderable!
    type = params[:folderable_type].presence || "Document"
    raise ActiveRecord::RecordNotFound unless ALLOWED_FOLDERABLE_TYPES.include?(type)

    case type
    when "Document"         then Current.workspace.documents.find(params[:folderable_id])
    when "AuthoredDocument" then Current.workspace.authored_documents.find(params[:folderable_id])
    when "EmailMessage"     then EmailMessage.accessible_to(Current.user).find(params[:folderable_id])
    end
  end

  # Only the documents page consumes the Turbo Stream (it files Documents via Turbo);
  # the Files area files everything non-Turbo, so it lands on the html redirect_back.
  def respond_after(folderable)
    respond_to do |format|
      if folderable.is_a?(Document)
        format.turbo_stream { render turbo_stream: folders_stream(folderable) }
      else
        format.turbo_stream { redirect_back fallback_location: files_path }
      end
      format.html { redirect_back fallback_location: files_path }
    end
  end

  def publish_membership_event(folderable, folder, verb)
    if folderable.is_a?(EmailMessage)
      Events.publish("email.#{verb}", subject: folderable,
        payload: { "subject" => folderable.subject, "folder" => folder.name })
    else
      Events.publish("file.#{verb}", subject: folderable,
        payload: { "filename" => membership_label(folderable), "folder" => folder.name })
    end
  end

  def membership_label(folderable)
    folderable.try(:display_title) || folderable.try(:title) || folderable.to_s
  end

  def folders_stream(document)
    turbo_stream.replace(helpers.dom_id(document, :folders),
      partial: "documents/folders", locals: { document: document })
  end
end
