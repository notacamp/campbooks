class Documents::WrittenController < ApplicationController
  before_action :set_document, only: [ :show, :edit, :update ]

  def index
    @documents = Current.workspace.authored_documents.recent
  end

  def new
    @document = Current.workspace.authored_documents.new
  end

  def create
    @document = Current.workspace.authored_documents.new(document_params)
    @document.author = current_user

    if @document.save
      redirect_to written_document_path(@document), success: t(".created")
    else
      render :new, status: :unprocessable_entity
    end
  end

  def show
  end

  def edit
  end

  def update
    if @document.update(document_params)
      redirect_to written_document_path(@document), success: t(".updated")
    else
      render :edit, status: :unprocessable_entity
    end
  end

  private

  def set_document
    @document = Current.workspace.authored_documents.find(params[:id])
  end

  def document_params
    params.require(:authored_document).permit(:title, :html_content)
  end
end
