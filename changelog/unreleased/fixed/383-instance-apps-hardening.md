- **Instance apps are locked down further.** The database now refuses any change to an
  instance app by the running server other than noting when it was last used, however
  the table grows. A sign-out hint stops working 30 days after it expired. A server in
  production ignores loopback `http` redirects stored for an instance app. Signing out of
  an instance app also ends your Campbooks session, and the docs now say so.
