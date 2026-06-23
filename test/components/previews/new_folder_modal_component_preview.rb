# frozen_string_literal: true

class NewFolderModalComponentPreview < Lookbook::Preview
  # The "New folder" dialog, open. In the app it sits inside the inbox folder
  # bar's `new-folder` controller; the "+" chip opens it and the form POSTs to
  # MailFoldersController#create.
  def default
    render(Campbooks::NewFolderModal.new(open: true))
  end

  # With a server-side validation error filled into #new_folder_error.
  def with_error
    render(Campbooks::NewFolderModal.new(open: true, error: "Name has already been taken"))
  end
end
