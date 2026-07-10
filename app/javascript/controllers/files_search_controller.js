// Drives the Files-area search bar. Inherits all behaviour (targets, values,
// actions) from EmailSearchController; the Stimulus identifier is `files-search`,
// so data attributes use the `files-search` prefix in views and templates.
import EmailSearchController from "controllers/email_search_controller"
export default class extends EmailSearchController {}
